-- Add migration script here
CREATE TABLE subscription_tokens (
    subscriber_id SERIAL REFERENCES subscriptions(id) ON DELETE CASCADE,
    subscription_token TEXT NOT NULL,
    PRIMARY KEY (subscription_token)
)