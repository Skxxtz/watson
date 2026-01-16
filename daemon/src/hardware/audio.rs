use std::sync::Arc;

use common::{
    protocol::{DaemonService, InternalMessage},
    utils::errors::{WatsonError, WatsonErrorKind},
    watson_err,
};
use libpulse_binding::{
    context::{Context, FlagSet},
    volume::{ChannelVolumes, Volume},
};
use tokio::sync::{Notify, mpsc, oneshot};

use crate::{DAEMON_TX, hardware::HardwareController, service_reg::ServiceRegister};

#[derive(Debug)]
pub struct VolumeState {
    tx: mpsc::Sender<AudioCommand>,
}
impl VolumeState {
    pub fn new(tx: mpsc::Sender<AudioCommand>) -> Self {
        Self { tx }
    }
}

#[derive(Debug)]
pub enum AudioCommand {
    SetVolume(u8),
    GetVolume { resp: oneshot::Sender<u8> },
    VolumeFetch { index: u32 },
}

impl HardwareController {
    // ----- Volume -----
    pub async fn set_volume(&mut self, percent: u8) -> Result<(), WatsonError> {
        let _permit = match &self.throttle.try_acquire() {
            Ok(p) => p,
            Err(_) => return Ok(()),
        };

        if let Some(state) = &self.volume_state {
            state
                .tx
                .send(AudioCommand::SetVolume(percent))
                .await
                .map_err(|e| watson_err!(WatsonErrorKind::StreamWrite, e.to_string()))?;
        }

        Ok(())
    }

    pub async fn get_volume(&mut self) -> Result<u8, WatsonError> {
        if let Some(state) = &self.volume_state {
            let (tx, rx) = oneshot::channel::<u8>();
            state
                .tx
                .send(AudioCommand::GetVolume { resp: tx })
                .await
                .map_err(|e| watson_err!(WatsonErrorKind::StreamWrite, e.to_string()))?;

            return rx
                .await
                .map_err(|_| watson_err!(WatsonErrorKind::Audio, "AudioActor rejected request."));
        } else {
            println!("test");
        }

        Ok(0)
    }
}

pub async fn audio_actor(
    tx: mpsc::Sender<AudioCommand>,
    mut rx: mpsc::Receiver<AudioCommand>,
    wake_signal: Arc<Notify>,
    register: Arc<ServiceRegister>,
) {
    use libpulse_binding::mainloop::threaded::Mainloop;
    let mut mainloop = Mainloop::new().unwrap();
    let mut ctx = Context::new(&mainloop, "WatsonDaemon").unwrap();
    ctx.connect(None, FlagSet::NOAUTOSPAWN, None).unwrap();

    mainloop.start().unwrap();
    loop {
        match ctx.get_state() {
            libpulse_binding::context::State::Unconnected => {}
            libpulse_binding::context::State::Connecting => {}
            libpulse_binding::context::State::Authorizing => {}
            libpulse_binding::context::State::SettingName => {}
            libpulse_binding::context::State::Ready => break,
            libpulse_binding::context::State::Failed => unimplemented!(),
            libpulse_binding::context::State::Terminated => unimplemented!(),
        }
    }
    println!("Pulse Audio Connected.");

    // Event listener for audio events
    ctx.subscribe(
        libpulse_binding::context::subscribe::InterestMaskSet::SINK,
        |_| {},
    );
    ctx.set_subscribe_callback(Some(Box::new({
        move |facility, _operation, index| {
            if facility == Some(libpulse_binding::context::subscribe::Facility::Sink) {
                let _ = tx.try_send(AudioCommand::VolumeFetch { index });
            }
        }
    })));

    let mut last_percentage: u8 = 0;
    loop {
        // Ghost check
        loop {
            if register.is_active(DaemonService::AudioService) {
                break;
            }
            println!("AudioService not registered.");

            wake_signal.notified().await;
        }

        tokio::select! {
            Some(cmd) = rx.recv() => {
                match cmd {
                    AudioCommand::SetVolume(v) => {
                        if v != last_percentage {
                            last_percentage = v;
                            let mut cv = ChannelVolumes::default();
                            let val = ((v as f64 / 100.0) * Volume::NORMAL.0 as f64) as u32;
                            cv.set(2, Volume(val));
                            ctx.introspect().set_sink_volume_by_name("@DEFAULT_SINK@", &cv, None);
                            mainloop.signal(false);
                        }
                    }
                    AudioCommand::GetVolume { resp } => {
                        let mut resp_opt = Some(resp);
                        ctx.introspect().get_sink_info_by_name("@DEFAULT_SINK@", move |info| {
                            if let libpulse_binding::callbacks::ListResult::Item(i) = info {
                                let percent = ((i.volume.avg().0 as f64 / Volume::NORMAL.0 as f64) * 100.0) as u8;
                                last_percentage = percent;
                                if let Some(r) = resp_opt.take() {
                                    let _ = r.send(percent);
                                }
                            }
                        });
                        mainloop.signal(false);
                    }
                    AudioCommand::VolumeFetch { index } => {
                        ctx.introspect().get_sink_info_by_index(index, move |info| {
                            if let libpulse_binding::callbacks::ListResult::Item(item) = info {
                                let avg_vol = item.volume.avg().0;
                                let percentage = ((avg_vol as f64 / Volume::NORMAL.0 as f64) * 100.0) as u8;
                                if percentage != last_percentage {
                                    last_percentage = percentage;
                                    let _result = DAEMON_TX.get().map(|d| d.send(InternalMessage::VolumeStateChange { percentage }));
                                }
                            }
                        });
                        mainloop.signal(false);
                    }
                }
            }
            else => break,
        }
    }
}
