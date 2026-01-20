# Sentinel

A Raspberry Pi based monitoring / early warning system.
It manages building security for our storage building, and general server monitoring at our offices.

At its core, it's a set of communication providers that send out alerts to configured recipients.
It supports [Pushover](https://pushover.net) and [sms-server](https://github.com/morgverd/sms-server) (via [sms-client](https://github.com/morgverd/sms-client))

### Sources

The system receives webhook events on `/cctv` and will then send out an alert.

- [CCTV](https://github.com/morgverd/cctv-smtp-alerts)

Unreleased but coming: iDrac support via SMTP server.

### Monitors

The system can also send alerts if a monitor fails, this is used to verify that everything is running fine.

- `cctv` - Verify that the DVR is still online within the network.
- `internet` - Verify that there is still an internet connection to send alarm notifications.
- `power` - **TODO**: Verify that the Pi still has a direct power connection (as it runs through a battery).
- `services` - Verify that the other local systemctl services are running (CCTV SMTP, Alarm Modem)
- `cron` - Send a GET request to CRON URL per CRON interval. Useful for health-checks / Sentry CRON.
