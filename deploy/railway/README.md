# Publishing the Railway template (owner)

1. In Railway, open the project that runs havenkeys-server (Postgres + server).
2. Project → Settings → **Generate Template from Project**.
3. In the template editor:
   - Postgres service: keep as is.
   - Server service: source = this GitHub repo, root directory empty (uses `Dockerfile` and `railway.json`).
   - Variables:
     - `DATABASE_URL` = `${{Postgres.DATABASE_URL}}?sslmode=disable`
     - `SERVER_SECRET` = empty, with the description "Run `openssl rand -base64 32` and paste the result" (the server requires base64 of exactly 32 bytes, which Railway's generators don't produce; Railway prompts the deployer for empty variables)
     - `HAVENKEYS_TRUST_FORWARDED_FOR` = `1`
     - `PORT` = `8080`
   - Networking: public domain on port 8080.
4. Publish. Copy the template URL (https://railway.com/deploy/...).
5. Put it in `apps/web/src/lib/links.ts` as `RAILWAY_TEMPLATE_URL` and deploy the site.
