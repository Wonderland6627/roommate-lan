# Updater signing keys

Generate a keypair (do this once per product, then back up the private key):

```powershell
npm run tauri signer generate -- -w src-tauri/keys/updater.key
```

- Put the **public** key contents into `src-tauri/tauri.conf.json` → `plugins.updater.pubkey`.
- Store the **private** key (and password) as GitHub Actions secrets:
  - `TAURI_SIGNING_PRIVATE_KEY`
  - `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`
- Never commit `*.key` files. Losing the private key breaks updates for already-installed clients.
