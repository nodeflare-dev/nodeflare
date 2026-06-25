-- Memory-weighted usage accumulator for per-server usage billing.
--
-- The existing `active_hours` column is plain machine hours (memory-blind). For
-- usage billing the cost driver is GB-hours = (memory_mb / 1024) * hours, so we
-- accumulate memory-weighted *minutes* here at sample time. Storing minutes (not
-- hours) keeps the sampler interval (e.g. 5 min) exact, and weighting at sample
-- time means a mid-month memory change is billed correctly. GB-hours = gb_minutes / 60.
ALTER TABLE region_usage ADD COLUMN gb_minutes DOUBLE PRECISION NOT NULL DEFAULT 0;
