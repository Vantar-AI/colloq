# colloq.dev

The Colloq landing page. Static files served by a Cloudflare Worker with assets.

```bash
# Node 22 is required for wrangler.
export NVM_DIR="$HOME/.nvm"; . "$NVM_DIR/nvm.sh"; nvm use 22

python3 -m http.server 4173 --directory .   # preview
npx wrangler@4 deploy                        # deploy to colloq.dev and www.colloq.dev
```

- No framework and no build step. One `index.html`, one stylesheet, three self-hosted
  variable fonts (Archivo, Inter, JetBrains Mono).
- `404.html` is required. Without it, Cloudflare answers every unknown path with 200.
- Every number on the page comes from the repo: the benchmark table in `docs/benchmark.md`,
  the Jev smoke run in `docs/jev.md`, and the test count from `cargo test`.
  Update the page when those change.
