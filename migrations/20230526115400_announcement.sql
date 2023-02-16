-- perform migration to add announcement feed
-- we store the last_updated_time as unix epoch (in UTC milliseconds)
CREATE TABLE IF NOT EXISTS announcement_feed
(
	id					TEXT		PRIMARY KEY NOT NULL,
	last_updated_time	INTEGER		NOT NULL
);
