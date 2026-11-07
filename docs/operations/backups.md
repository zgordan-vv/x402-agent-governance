# x402 MVP backups

On the shared DigitalOcean host, x402 PostgreSQL backups are written to `/var/backups/x402-governance` by `/usr/local/sbin/x402-governance-backup` at 03:15 UTC. The backup check runs at 03:30 UTC and writes an alert-level syslog event if no dump was created in 26 hours. Dumps older than 14 days are removed from the host.

Manual verification:

```bash
ssh admin.codehummus.com
sudo /usr/local/sbin/x402-governance-backup
sudo /usr/local/sbin/x402-governance-backup-check
sudo ls -lh /var/backups/x402-governance
```

The restore rehearsal creates a temporary `governance_restore_check` database, restores the newest dump, checks the workspace table, and drops the temporary database. Backups are currently host-local MVP copies; production requires encrypted off-host/object-storage copies and an alert delivery channel.
