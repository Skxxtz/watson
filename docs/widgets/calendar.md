# Calendar

## Credential Storage and Security

Watson stores calendar credentials encrypted on the local system to protect against accidental disclosure and casual inspection (for example via backups or configuration files).

Credentials are encrypted using a randomly generated master key and modern authenticated encryption. The master key is stored locally and is not transmitted or shared. This design allows Watson to retrieve calendar data unattended after login, without requiring additional user interaction or desktop-specific keyring services.

This approach provides protection for data at rest but does not protect against malware running under the user account, a compromised system, or direct memory access while the application is running.

Watson does not rely on external credential managers or operating-system keyrings to ensure predictable behavior across environments.

Watson uses XChaCha20-Poly1305 for password encryption.

## Supported Services

### iCloud

Watson can automatically connect to your iCloud calendars. For this to work, it
needs (1) your AppleID, and (2) an app-specific password.

#### Configuration on iCloud

1. Go to icloud.com
2. Sign-In
3. Click on your profile picture
4. Click "Manage Apple Account"
5. Go to "Sign-In and Security"
6. Click "App-Specific Passwords"
7. Create new password. (Give it a name like "watson")

#### Configuration in Watson

1. Launch Watson's Authentication TUI with:
```bash
watson auth
```
2. Choose `Configure New Account` from the menu.
3. Select ***iCloud*** as the service
4. Enter your **Apple ID** and the **app-specific password** you created on iCloud.
5. Give the account a descriptive label (e.g., "Personal iCloud").
6. Save your credentials by pressing `<RETURN>`. 
