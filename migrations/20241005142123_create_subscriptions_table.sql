-- Add migration script here
-- 创建订阅表
DROP TABLE IF EXISTS subscriptions;
CREATE TABLE subscriptions(
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    email TEXT NOT NULL UNIQUE,
    name TEXT NOT NULL,
    subscribed_at INTEGER NOT NULL
);