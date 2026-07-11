# Dislight

A focused Windows 1:1 chat client with direct voice channels, screen sharing, temporary media, local media retention, and a shared soundboard.

## Stack

- React 19, TypeScript, Vite, Tailwind CSS
- Tauri 2 for the Windows desktop shell
- Supabase Auth, Postgres, Realtime, Storage, and Edge Functions

## Local setup

1. Install Node.js and Rust.
2. Copy .env.example to .env.
3. Set VITE_SUPABASE_URL and VITE_SUPABASE_ANON_KEY.
4. Run npm install.
5. Run npm run tauri -- dev.

## Builds

- Web: npm run build
- Windows installers: npm run tauri -- build

Generated installers are written under src-tauri/target/release/bundle/.

## Vercel

The web client builds to dist. Configure the same two public Supabase variables in Vercel for Production, Preview, and Development environments.

## Storage budgets

Dislight keeps Supabase Free usage bounded with separate server-enforced budgets for temporary chat media and permanent soundboard clips. Avatars overwrite in place.