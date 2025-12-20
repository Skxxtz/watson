# Calendar

## ICloud

Watson can automatically connect to your ICloud calendars. For this to work, it
needs (1) your AppleID, and (2) an app-specific password.

### Configuration on ICloud

1. Go to icloud.com
2. Sign-In
3. Click on your profile picture
4. Click "Manage Apple Account"
5. Go to "Sign-In and Security"
6. Click "App-Specific Passwords"
7. Create new password. (Give it a name like "watson")

#### Configuration in Watson

> [!IMPORTANT]
> Watson stores your credentials securely using your system's keyring service.
> Ensure it is installed and running before proceeding. 


1. Launch Watson's Authentication TUI with:
```bash
watson auth
```
2. Choose `Configure New Account` from the menu.
3. Select ***ICloud*** as the service
4. Enter your **Apple ID** and the **app-specific password** you created on iCloud.
5. Give the account a descriptive label (e.g., "Personal ICloud").
6. Save your credentials by pressing `<RETURN>`. Watson will now securely store
   them in the system keyring and use them to access your calendar events
   automatically.
