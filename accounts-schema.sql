CREATE TABLE IF NOT EXISTS "Group" (
	"id"	INTEGER NOT NULL,
	"groupname"	TEXT NOT NULL UNIQUE,
	PRIMARY KEY("id" AUTOINCREMENT)
);
CREATE INDEX "Group_groupname" ON "Group" (
	"groupname"
);
CREATE TABLE IF NOT EXISTS "UserInGroup" (
	"user_id"	INTEGER NOT NULL,
	"group_id"	INTEGER NOT NULL,
	FOREIGN KEY("user_id") REFERENCES "User"("id"),
	FOREIGN KEY("group_id") REFERENCES "Group"("id")
);
CREATE INDEX "UserInGroup_user_id" ON "UserInGroup" (
	"user_id"
);
CREATE INDEX "UserInGroup_group_id" ON "UserInGroup" (
	"group_id"
);
CREATE TABLE IF NOT EXISTS "User" (
	"id"	INTEGER NOT NULL,
	"username"	TEXT NOT NULL UNIQUE,
	"email"	TEXT,
	"name"	TEXT,
	"surname"	TEXT,
	"hashed_pass"	BLOB,
	PRIMARY KEY("id" AUTOINCREMENT)
);
CREATE INDEX "User_username" ON "User" (
	"username"
);
CREATE INDEX "User_email" ON "User" (
	"email"
);
CREATE TABLE IF NOT EXISTS "FailedLoginAttempt" (
	"id"	INTEGER NOT NULL,
	"ip"	BLOB NOT NULL,
	"username"	TEXT NOT NULL,
	"unixtime_next_allowed"	INTEGER NOT NULL,
	"seconds_next_wait"	REAL NOT NULL,
	PRIMARY KEY("id")
);
CREATE INDEX "LoginAttempt_ip" ON "FailedLoginAttempt" (
	"ip"
);
CREATE INDEX "LoginAttempt_username" ON "FailedLoginAttempt" (
	"username"
);
CREATE TABLE IF NOT EXISTS "SessionData" (
	"id"	INTEGER NOT NULL,
	"sessionid_hash"	BLOB NOT NULL UNIQUE,
	"last_request_time"	INTEGER NOT NULL,
	"user_id"	INTEGER,
	"ip"	BLOB,
	FOREIGN KEY("user_id") REFERENCES "User"("id"),
	PRIMARY KEY("id" AUTOINCREMENT)
);
CREATE INDEX "SessionData_last_request_time" ON "SessionData" (
	"last_request_time"
);
CREATE INDEX "SessionData_sessionid" ON "SessionData" (
	"sessionid_hash"
);
CREATE INDEX "SessionData_user_id" ON "SessionData" (
	"user_id"
);
