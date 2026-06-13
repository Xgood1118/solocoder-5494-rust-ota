-- 设备表
CREATE TABLE IF NOT EXISTS devices (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    device_id TEXT NOT NULL UNIQUE,
    device_type TEXT NOT NULL,
    firmware_version TEXT,
    hardware_version TEXT,
    battery_level INTEGER,
    wifi_ssid TEXT,
    wifi_signal INTEGER,
    last_heartbeat_at DATETIME,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_devices_device_type ON devices(device_type);

-- 固件表
CREATE TABLE IF NOT EXISTS firmware (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    version TEXT NOT NULL,
    device_type TEXT NOT NULL,
    file_path TEXT NOT NULL,
    file_size INTEGER NOT NULL,
    sha256_hash TEXT NOT NULL,
    rsa_signature TEXT NOT NULL,
    metadata TEXT,
    description TEXT,
    is_active INTEGER NOT NULL DEFAULT 0,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_firmware_device_type ON firmware(device_type);
CREATE UNIQUE INDEX IF NOT EXISTS idx_firmware_version_type ON firmware(version, device_type);

-- 升级任务表
CREATE TABLE IF NOT EXISTS upgrade_tasks (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    task_name TEXT NOT NULL,
    firmware_id INTEGER NOT NULL,
    device_type TEXT NOT NULL,
    gray_stage INTEGER NOT NULL DEFAULT 0,
    gray_percentages TEXT NOT NULL DEFAULT '[5,20,50,100]',
    min_battery_level INTEGER,
    require_wifi INTEGER NOT NULL DEFAULT 0,
    time_window_start TEXT,
    time_window_end TEXT,
    failure_rate_threshold REAL NOT NULL DEFAULT 0.1,
    status TEXT NOT NULL DEFAULT 'pending',
    auto_paused INTEGER NOT NULL DEFAULT 0,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (firmware_id) REFERENCES firmware(id)
);

CREATE INDEX IF NOT EXISTS idx_upgrade_tasks_status ON upgrade_tasks(status);
CREATE INDEX IF NOT EXISTS idx_upgrade_tasks_device_type ON upgrade_tasks(device_type);

-- 设备升级记录表
CREATE TABLE IF NOT EXISTS device_upgrades (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    task_id INTEGER NOT NULL,
    device_id TEXT NOT NULL,
    firmware_id INTEGER NOT NULL,
    status TEXT NOT NULL DEFAULT 'pending',
    gray_stage INTEGER NOT NULL DEFAULT 0,
    failure_reason TEXT,
    started_at DATETIME,
    completed_at DATETIME,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (task_id) REFERENCES upgrade_tasks(id),
    FOREIGN KEY (firmware_id) REFERENCES firmware(id)
);

CREATE INDEX IF NOT EXISTS idx_device_upgrades_task_id ON device_upgrades(task_id);
CREATE INDEX IF NOT EXISTS idx_device_upgrades_device_id ON device_upgrades(device_id);
CREATE INDEX IF NOT EXISTS idx_device_upgrades_status ON device_upgrades(status);
CREATE UNIQUE INDEX IF NOT EXISTS idx_device_upgrades_task_device ON device_upgrades(task_id, device_id);

-- 状态上报历史表
CREATE TABLE IF NOT EXISTS status_reports (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    device_id TEXT NOT NULL,
    task_id INTEGER,
    status TEXT NOT NULL,
    firmware_version TEXT,
    progress INTEGER,
    error_message TEXT,
    battery_level INTEGER,
    wifi_signal INTEGER,
    reported_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_status_reports_device_id ON status_reports(device_id);
CREATE INDEX IF NOT EXISTS idx_status_reports_task_id ON status_reports(task_id);
CREATE INDEX IF NOT EXISTS idx_status_reports_reported_at ON status_reports(reported_at);

-- HMAC 签名密钥表（双 key 灰度轮换）
CREATE TABLE IF NOT EXISTS hmac_keys (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    key_id TEXT NOT NULL UNIQUE,
    secret_key TEXT NOT NULL,
    is_active INTEGER NOT NULL DEFAULT 0,
    is_primary INTEGER NOT NULL DEFAULT 0,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP,
    expires_at DATETIME
);

-- 审计日志表
CREATE TABLE IF NOT EXISTS audit_logs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    action TEXT NOT NULL,
    actor TEXT NOT NULL,
    resource_type TEXT NOT NULL,
    resource_id TEXT,
    old_value TEXT,
    new_value TEXT,
    ip_address TEXT,
    user_agent TEXT,
    created_at DATETIME NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_audit_logs_action ON audit_logs(action);
CREATE INDEX IF NOT EXISTS idx_audit_logs_resource ON audit_logs(resource_type, resource_id);
CREATE INDEX IF NOT EXISTS idx_audit_logs_created_at ON audit_logs(created_at);

-- 初始化 HMAC 密钥
INSERT OR IGNORE INTO hmac_keys (key_id, secret_key, is_active, is_primary)
VALUES ('key-primary', 'replace-with-your-primary-secret-key-please-change-in-production', 1, 1);

INSERT OR IGNORE INTO hmac_keys (key_id, secret_key, is_active, is_primary)
VALUES ('key-secondary', 'replace-with-your-secondary-secret-key-please-change-in-production', 1, 0);
