STATE state FOR LEADING OBJECT TYPE 'Machine' AS CASE
  WHEN event.data_complete = false THEN 'Unknown'
  WHEN event.down_active = true OR event.mode = 'DOWN' THEN 'Down'
  WHEN event.quality_hold_active = true THEN 'Quality Hold'
  WHEN event.recovery_active = true THEN 'Recovery'
  WHEN event.mode = 'SETUP' THEN 'Setup'
  WHEN event.degraded_latched = true THEN 'Degraded'
  WHEN event.mode = 'RUNNING' THEN 'Running'
  ELSE 'Idle'
END
