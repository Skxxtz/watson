use common::{
    errors::{WatsonError, WatsonErrorKind},
    watson_err,
};
use libpulse_binding::{
    context::{Context, FlagSet},
    volume::{ChannelVolumes, Volume},
};
use tokio::sync::{mpsc, oneshot};

use crate::hardware::HardwareController;

pub struct VolumeState {
    tx: mpsc::Sender<AudioCommand>,
}

#[derive(Debug)]
enum AudioCommand {
    SetVolume(u8),
    GetVolume { resp: oneshot::Sender<u8> },
}

impl HardwareController {
    // ----- Volume -----
    fn set_volume_state(&mut self) {
        let (tx, rx) = mpsc::channel::<AudioCommand>(16);
        std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap();

            rt.block_on(async move { audio_actor(rx).await });
        });
        self.volume_state.replace(VolumeState { tx });
    }

    pub async fn set_volume(&mut self, percent: u8) -> Result<(), WatsonError> {
        if self.volume_state.is_none() {
            self.set_volume_state();
        }

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
        if self.volume_state.is_none() {
            self.set_volume_state();
        }

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
        }

        Ok(0)
    }
}

async fn audio_actor(mut rx: mpsc::Receiver<AudioCommand>) {
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

    loop {
        tokio::select! {
            // This is your "Tokio Arm" for the receiver
            Some(cmd) = rx.recv() => {
                match cmd {
                    AudioCommand::SetVolume(v) => {
                        let mut cv = ChannelVolumes::default();
                        let val = ((v as f64 / 100.0) * Volume::NORMAL.0 as f64) as u32;
                        cv.set(2, Volume(val));
                        ctx.introspect().set_sink_volume_by_name("@DEFAULT_SINK@", &cv, None);
                        mainloop.signal(false);
                    }
                    AudioCommand::GetVolume { resp } => {
                        let mut resp_opt = Some(resp);
                        ctx.introspect().get_sink_info_by_name("@DEFAULT_SINK@", move |info| {
                            if let libpulse_binding::callbacks::ListResult::Item(i) = info {
                                let percent = ((i.volume.avg().0 as f64 / Volume::NORMAL.0 as f64) * 100.0) as u8;
                                if let Some(r) = resp_opt.take() {
                                    let _ = r.send(percent);
                                }
                            }
                        });
                        mainloop.signal(false);
                    }
                }
            }
            // You can add more arms here later (e.g., event subscriptions)
            else => break, // Exit if the channel is closed
        }
    }
}
