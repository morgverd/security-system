# Security System

My personal security system that runs on a Raspberry Pi across multiple buildings.

It runs a set of monitors to track important states, such as power connection etc.
It also runs a HTTP server to accept webhooks from the CCTV SMTP and Alarm Modem.

### Sources

The system receives webhook events on either `/cctv` or `/alarm` and will then send out an alert.

- CCTV: https://github.com/morgverd/cctv-smtp-alerts
- Alarm: https://github.com/morgverd/alarm-modem

### Monitors

The system can also send alerts if a monitor fails, this is used to verify that everything is running fine.

- `cctv` - Verify that the DVR is still online within the network.
- `internet` - Verify that there is still an internet connection to send alarm notifications.
- `power` - Verify that the Pi still has a direct power connection (as it runs through a battery).
- `processes` - Verify that the other local processes are running (CCTV SMTP, Alarm Modem)

### Alerts

The system uses Pushover to send notifications to a group.

**Goal**: Use a SIM card to send text messages if there is no internet connection (eg: building lost power).