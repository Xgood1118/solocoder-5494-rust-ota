-- Add device status field for lifecycle management
ALTER TABLE devices ADD COLUMN status TEXT NOT NULL DEFAULT 'active';

-- Add index on status for filtering
CREATE INDEX idx_devices_status ON devices(status);
