-- Add migration script here
INSERT INTO users (user_id, username, password_hash)
VALUES (
'd3379c53-2bd2-4828-be81-69b67f37cf85',
'admin',
'$argon2id$v=19$m=15000,t=2,p=1$9wBXVcF9oIEsBGv6yU+A8A$uvpN6dcZ9V6gMFkcQ7fMY4jhF2WmoaqxujYk2T0EQl8'
);