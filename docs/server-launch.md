# Launching the D.A.D server

The owner's steps, in order. Written 2026-11-25 on branch `dad/server-launch`. Nothing here has been run against a real account.

**What launches:** the Rust server `server/api` as a **second instance on the VPS** that already runs verSHion. It gets its own
hostname, database, secrets and AI budget. D.A.D AI then works for every teacher who downloads a release: they sign in and
spend credits, with no key. The Cloudflare Worker (`server/worker`) is **not** used; see "Not chosen" at the end.

**Proven locally (2026-11-25):** a debug D.A.D build against `server/api` on this machine, with a local model as the
upstream. Sign-up, the emailed link, and sign-in all worked. D.A.D AI was first in the AI list. A question was written and
passed 3 checks for 1 credit, and a rewrite cost 1 more. The app's sentences for out of credits, server down and sign-in
ended were checked too. This needed `6a28374d` (server: the `question` service, text replies) and `7fa60ec8` (app: opens
the credit session). A server without `6a28374d` refuses every D.A.D question.

Example hostname below: `dad.lefthanddev.com`. Pick yours in step 1 and use it everywhere.

## Costs at a glance

| Item | Cost |
|---|---|
| VPS, Docker, Caddy, Cloudflare tunnel and DNS | $0 extra: all already in place for verSHion |
| Hostname on `lefthanddev.com` | $0 (a subdomain of a domain you already pay for) |
| AI upstream (CheaperInference) | Pay per use. `AI_DAILY_USD` caps it per UTC day: $1 a day means at most about $30 a month |
| Mail | Cloudflare Email Sending (what verSHion production uses): check its current price before you enable it. Stalwart on the VPS costs $0, but Gmail rate-limits that IP block (docs/hosting-port.md, finding B) |
| Stripe (buying credits) | $0 while `BILLING_ENABLED=0`. Stripe fees per sale once it's on |
| GitHub Actions variables | $0 |

## What can't be undone

- **`POOL_HASH_KEY`** can never change once the database holds shared items. Back it up offline (step 6).
- **A released build keeps its server address forever.** Every downloaded copy calls `VERSHION_POOL_URL`. If the hostname
  changes later, old copies lose D.A.D AI. Choose the hostname once (step 1).
- **Accounts hold teachers' email addresses** from the first sign-up. The privacy page's promises apply from then on.
- Changing `AUTH_SECRET` later signs everyone out. That can be undone, but every teacher has to sign in again.

## 1. Decide (free)

- [ ] **Hostname.** For example `dad.lefthanddev.com`. It must be on a domain in the same Cloudflare account as the VPS
      tunnel.
- [ ] **Sharing stays closed.** `VERSHION_POOL_URL` also switches on D.A.D's Share and Browse-the-pool screens. The pool
      lets a teacher in only after an admin approves them and they redeem an invite code. Approve nobody for the pool, and
      only D.A.D AI is live. Opening the pool brings the legal work in docs/handoff-shared-pool.md ("My decisions": DMCA
      agent, US$6 every 3 years, and a lawyer review).
- [ ] **Daily AI cap.** Recommend `AI_DAILY_USD=1` for the first weeks; raise it once you've seen a week of real use.
      Each teacher gets 40 free credits a week by default (`AI_WEEKLY_CREDITS`). A question or a rewrite costs 1 credit.

## 2. AI key (paid per use)

- [ ] Create a **new** CheaperInference API key for D.A.D. Keep it separate from verSHion's so the two bills stay apart.
      Set a spending limit in their dashboard if they offer one.
- [ ] Don't paste it anywhere yet. It goes only into the server's env file (step 5).

## 3. Mail (see the cost table)

- [ ] Cloudflare Email Sending: make a **separate** API token named for D.A.D (Email Sending: Edit). If the sending domain
      is `lefthanddev.com`, the existing SPF and DKIM records already cover it.
- [ ] Note the sender, for example `noreply@lefthanddev.com`, and the admin address that should get alerts.
- Note: the server's mails say "verSHion" (for example "Confirm your verSHion address"). They are written in
  `server/api/src/auth/smtp.rs` and `notices.rs`. Renaming them is a code change, not a setting.

## 4. Build and copy (free; run from the repo root on this machine)

The server binary is built in Debian 12, so it runs on the VPS's glibc:

```
docker run --rm -v "$PWD/server:/w/server" -v "$PWD/prompts:/w/prompts:ro" -v dad-target:/target \
  -e CARGO_TARGET_DIR=/target -w /w/server/api rust:1-bookworm cargo build --release
mkdir -p out && docker run --rm -v dad-target:/target -v "$PWD/out:/out" alpine cp /target/release/vershion-api /out/
```

Web root: the link page, the legal pages and the credits page. The link page tells a teacher who opened their emailed
link in a browser to paste it into D.A.D.

```
mkdir -p out/web && cp server/web/dad-link/index.html out/web/index.html
cp -r server/web/legal out/web/legal && rm out/web/legal/README.md out/web/legal/fill.py
python3 server/web/legal/fill.py out/web/legal          # edit OPERATOR, EMAIL, ... at the top of fill.py first
cp -r server/web/account out/web/account
sed -i 's#https://api.example.org#https://dad.lefthanddev.com#' out/web/account/credits.html
```

The legal pages still describe the verSHion pool and carry a DRAFT banner. Read them before launch.

Copy to the VPS:

```
ssh vps 'sudo install -d -o debian -g debian /opt/dad-server /var/www/dad-server && sudo install -d -o 10001 -g 10001 /var/lib/dad-server && sudo install -d -m 700 /etc/dad-server'
sed 's/vershion-staging/dad-server/g' server/api/staging/compose.yml > out/compose.yml
sed 's/vershion-staging/dad-server/g' server/api/staging/Caddyfile  > out/Caddyfile
rsync -a out/compose.yml out/Caddyfile server/api/staging/Dockerfile out/vershion-api vps:/opt/dad-server/
rsync -a server/worker/migrations/ vps:/opt/dad-server/migrations/
rsync -a --delete out/web/ vps:/var/www/dad-server/
```

After the `sed`, the containers are `dad-server-api` and `dad-server-caddy`, on the existing `pangolin_frontend` network.
None of verSHion's containers or paths are touched.

## 5. Secrets and settings (free; on the VPS)

Make the env file with two fresh secrets. The values are generated on the server and never printed:

```
sudo sh -c 'umask 077; f=/etc/dad-server/api.env; for k in AUTH_SECRET POOL_HASH_KEY AI_RELAY_KEY; do
  printf "%s=%s\n" "$k" "$(head -c48 /dev/urandom | base64 | tr -d "/+=\n")" >> "$f"; done'
sudoedit /etc/dad-server/api.env
```

Add these lines in the editor. Paste the two secret values (`CHEAPERINFERENCE_API_KEY`, `SMTP_PASSWORD`) by hand; they
never go through a command line:

```
ALLOWED_ORIGINS=https://dad.lefthanddev.com
APP_URL=https://dad.lefthanddev.com
SMTP_HOSTNAME=smtp.mx.cloudflare.net
SMTP_PORT=465
SMTP_SECURE=1
SMTP_USERNAME=api_token
SMTP_PASSWORD=<the step-3 token>
MAIL_FROM=noreply@lefthanddev.com
SMTP_FROM_NAME=D.A.D
ADMIN_EMAIL=<your address>
CHEAPERINFERENCE_API_KEY=<the step-2 key>
AI_DAILY_USD=1
BILLING_ENABLED=0
```

- **`AI_RELAY_KEY` is required.** Without it, a request with no sign-in, or with an expired one, gets free AI as an
  anonymous caller; only the hourly and daily caps stop it. This was seen locally on 2026-11-25 and is finding H7 in
  docs/hosting-port.md. Nobody needs its value: D.A.D signs in instead.
- compose.yml sets `VERSHION_LISTEN`, `VERSHION_DB`, `VERSHION_BLOBS` and `VERSHION_MIGRATIONS` (the data ends up in
  `/var/lib/dad-server`). Don't add them here.

## 6. Back up the secrets (free; can't be skipped)

- [ ] Copy `/etc/dad-server/api.env` into your password manager, plus one offline copy. Losing `POOL_HASH_KEY` can't be
      repaired.
- [ ] Nightly database backup: copy `server/api/staging/vershion-backup` to `/usr/local/bin/dad-backup`, set
      `SRC=/var/lib/dad-server` and `DST=/var/backups/dad-server`, and give it its own timer. docs/backup-restore.md
      covers the R2 upload and the age key. R2 storage for a small SQLite file stays within Cloudflare's free allowance.

## 7. Start it (free)

```
ssh vps 'cd /opt/dad-server && docker compose up -d --build && docker compose logs api | tail -20'
ssh vps 'docker exec dad-server-caddy wget -qO- http://dad-server-api:8787/ai/health'
```

The health reply should have `"ai":true`. The log should show `listening` and the migrations being applied.

## 8. Front door (free; a second of downtime for the other hosts)

- [ ] In `/home/debian/.cloudflared/config.yml` on the VPS, add above the 404 rule:
      ```
        - hostname: dad.lefthanddev.com
          service: http://dad-server-caddy:80
      ```
      then `docker restart cloudflared`.
- [ ] In Cloudflare DNS, add a proxied CNAME `dad` pointing to `<tunnel-id>.cfargotunnel.com`.

## 9. Check it from outside (costs a few cents of AI)

```
curl -s https://dad.lefthanddev.com/ai/health                     # {"ai":true,...}
curl -s -o /dev/null -w '%{http_code}\n' -X POST https://dad.lefthanddev.com/ai/chat/completions \
  -H 'content-type: application/json' -d '{"messages":[{"role":"user","content":"hi"}]}'   # 401, not 200
curl -s -o /dev/null -w '%{http_code}\n' https://dad.lefthanddev.com/legal/terms.html          # 200
```

Then run a debug app against it: `VERSHION_POOL_URL=https://dad.lefthanddev.com cargo run --bin vershion`.
In Settings, then D.A.D account, create an account with a real address and confirm it from the mail. On Questions,
write one question. Settings should then show one credit fewer.

## 10. Release builds (free; can't be undone once a release is downloaded)

```
gh variable set VERSHION_POOL_URL  --body https://dad.lefthanddev.com --repo shuff57/D.A.D---DynamicAssessmentDevelopmer
gh variable set VERSHION_LEGAL_URL --body https://dad.lefthanddev.com --repo shuff57/D.A.D---DynamicAssessmentDevelopmer
```

- `VERSHION_LEGAL_URL` is the **site root**. The app adds `/legal/terms.html` and the other page names itself.
- `build.rs` refuses anything that isn't plain https. The next `desktop.yml` run, a push to `dad/engine` or a `v*` tag,
  builds with both variables.
- First merge this branch, with `6a28374d`, `7fa60ec8` and `47e87884`, into the branch you release from. Deploy the
  server from step 4 **before** you publish a release.

## Not chosen: the Cloudflare Worker

`server/worker` would need the Workers Paid plan (US$5 a month minimum; argon2 needs it), plus D1, R2, `wrangler d1
create`, a `[[routes]]` entry, the real `database_id` in `wrangler.toml`, and `wrangler secret put` for each secret.
It also lacks the `question` service and text replies that `6a28374d` added to `server/api`. Those would have to be ported
to `server/worker/src/ai/` first, or every D.A.D question is refused with 402.
