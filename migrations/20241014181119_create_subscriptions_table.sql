-- Add migration script here
-- 创建订阅表
CREATE TABLE subscriptions (
    id SERIAL PRIMARY KEY,
    email TEXT NOT NULL UNIQUE,
    name TEXT NOT NULL,
    subscribed_at TIMESTAMP NOT NULL
);