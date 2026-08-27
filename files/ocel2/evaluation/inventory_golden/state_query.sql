STATE state FOR LEADING OBJECT TYPE 'ItemLocation' AS CASE
  WHEN event.data_complete = false THEN 'Unknown'
  WHEN event.critical_understock = true THEN 'Critical Understock'
  WHEN event.on_hand_after < event.lower_threshold THEN 'Understock'
  WHEN event.on_hand_after > event.upper_threshold THEN 'Overstock'
  ELSE 'Normal'
END
