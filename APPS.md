# Contract of nas-backup-manager with the NAS launcher

The contract every application shares with the launcher (package, manifest, permissions, frame
protocol, launcher socket and providers) is `docs/development/external_apps/APPS.md` in the
nas-launcher repository, and the bridge is the one of `@valeriogc/nas-base-app`. This file keeps
only what is specific to nas-backup-manager, which builds and releases on its own.

## Identity

- id `nas-backup-manager`, category `storage`
- permissions `paths` ["/srv/storage"], `packages` ["7zip"]

## Specific to this application

nas-disk-manager declares the `network-disks` capability at `/api/network-disks`, which answers with the network shares mounted on the host (`cifs`, `smb3`, `nfs`, `nfs4`, `fuse.sshfs`) in the same shape as `/api/disks`. An application sends `{ nas: 1, type: 'network-disks', requestId }` through `requestNetworkDisks()`; the launcher starts the first installed version 1 provider if necessary, reads the endpoint and returns `{ nas: 1, type: 'network-disks-result', requestId, success, data?: [{ name, mount, source, readOnly }], error? }`. No application asks for it at present. nas-backup-manager reaches the network disks through the Explorer picker like the other disks, while its cloud backups wait for the cloud disks of nas-disk-manager and open that section with `location: '?kind=cloud'`.

Any application may read the power schedule of the NAS with `{ nas: 1, type: 'power-schedule', requestId }` through `requestPowerSchedule()`: the launcher answers `{ nas: 1, type: 'power-schedule-result', requestId, success, data?: { enabled, rules: [{ day, time, action }] }, error? }`, with only the active rules. nas-backup-manager uses it in the schedule step of a rule, to show when the NAS is off and to refuse a time that falls there.

The commands that check the repository are in [DEVELOPMENT.md](DEVELOPMENT.md).
