# Sentinel

A Raspberry Pi based monitoring / early warning system.
It manages building security for our storage building, and general server monitoring at our offices.

At its core, it's a set of communication providers that send out alerts to configured recipients.
It supports [Pushover](https://pushover.net) and [sms-server](https://github.com/morgverd/sms-server) (via [sms-client](https://github.com/morgverd/sms-client))

### Sources

The system receives webhook events on `/cctv` and will then send out an alert.

- CCTV - https://github.com/morgverd/cctv-smtp-alerts

Unreleased but coming: iDrac support via SMTP server.
