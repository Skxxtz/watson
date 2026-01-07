# Calendar Widget

A lightweight, performant calendar widget built for Watson. It provides a unified view of your schedule by integrating multiple cloud providers with a focus on security and ease of use.

## Features

* **Multi-Service Support**: Native integration with iCloud and Google Calendar.
* **Encrypted Storage**: Secure, local-first credential management.
* **Async Synchronization**: Non-blocking data fetching to keep the UI responsive.
* **Privacy Focused**: Direct connection to providers without intermediary servers.

---

## Security & Credential Storage

Watson prioritizes the security of your access tokens and passwords. Credentials are stored on the local system using **authenticated encryption (XChaCha20-Poly1305)** to prevent unauthorized access via backups or configuration file inspection.

### The Security Model
* **Master Key**: A randomly generated master key is stored locally to allow for unattended background refreshes after login.
* **Encrypted at Rest**: All service tokens and app-specific passwords are encrypted before being written to disk.
* **Independence**: Watson does not rely on external OS keyrings (like GNOME Keyring or KWallet). This ensures consistent behavior across different desktop environments and headless setups.

> [!IMPORTANT]
> This design protects data at rest but does not defend against active malware running under the same user account or direct memory access while the process is active.

---

## Supported Services

### ☁️ iCloud
Watson connects to iCloud via the CalDAV protocol. For security, Apple requires the use of an **App-Specific Password**.

#### 1. Generate an App-Specific Password
1. Sign in to [appleid.apple.com](https://appleid.apple.com).
2. Navigate to **Sign-In and Security** > **App-Specific Passwords**.
3. Click the **+** icon, enter a label (e.g., "Watson"), and click **Create**.
4. Copy the generated password.

#### 2. Configure Watson
Run the Authentication TUI:
```bash
watson auth
```
* Select **Configure New Account** > **iCloud**.
* Enter your **Apple ID** and the **App-Specific Password**.
* Assign a label (e.g., "Personal") and save.

---

### ☁️ Google Calendar
*Status: Implementation complete. Currently pending Google Application Verification.*

Google integration uses OAuth 2.0 for secure access without sharing your primary password.

#### Configuration
1. Run `watson auth` and select **Google**.
2. A browser window will open requesting access to your Google Calendar.
3. Once authorized, Watson will automatically receive and encrypt your access tokens.

> [!NOTE]
> Until Google verification is finalized, this service will not work. 

---
