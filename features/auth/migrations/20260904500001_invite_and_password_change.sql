-- Team-invite mechanism: `register()` remains a one-time bootstrap,
-- and GitLab OAuth (#19) never creates an account, so without this
-- the system could only ever have one user. Any authenticated
-- session can invite a new account with a server-generated temporary
-- password; `must_change_password` forces that password to be
-- replaced before the account can do anything but change it or read
-- its own profile.
ALTER TABLE users ADD COLUMN must_change_password BOOLEAN NOT NULL DEFAULT 0;
