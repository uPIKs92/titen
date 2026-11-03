-- 016: per-account Threads app credentials carried through the OAuth redirect.
--
-- The authorize-url request supplies (or inherits from an account) the Threads
-- app_id/app_secret to use for THIS connection. They ride on the one-time state
-- row so the exchange step can use the same app, then are saved on the account.
-- app_secret_enc is encrypted at rest like accounts.app_secret.
ALTER TABLE oauth_states ADD COLUMN app_id TEXT;
ALTER TABLE oauth_states ADD COLUMN app_secret_enc TEXT;
