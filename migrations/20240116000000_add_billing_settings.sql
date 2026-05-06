-- Add billing settings to workspaces
ALTER TABLE workspaces ADD COLUMN auto_email_invoices BOOLEAN DEFAULT TRUE;
