# Changelog

## Shurly

> Shurly, this is a URL shortener with API management

## Version 0.2.4

### Fixes

-   Upgrade to latest h2 (0.4.2) — Fixes GHSA-8r5v-vm4m-4g25

## Version 0.2.3

### Fixes

-   Upgrade to latest `axum` (0.7.3), with the new `hyper` release (1.1.0)
-   Update Github workflows with latest (maintained) actions

## Version 0.2.2

### Fixes

-   Upgrade to latest `rustix` (0.38.20), fixes security vulnerability GHSA-c827-hfw6-qwvm

## Version 0.2.1

### Features

-   Add unicode normalization to slugs
-   Upgrade all dependencies to their latest versions

### Fixes

-   Update MSRV to 1.70
-   Upgrade all dependencies to their latest versions
    -   SQLx has a new method for offline support
    -   No other notable changes

## Version 0.2.0

### Fixes

-   Update MSRV to 1.70
-   Upgrade all dependencies to their latest versions
    -   SQLx has a new method for offline support
    -   No other notable changes

## Version `0.1.2`

### Fixes

-   Upgrade to latest `axum` dependency (~0.5), fixes security vulnerability CVE-2022-3212

## Version `0.1.1`

### Features

-   Add user friendly error/not found pages

## Initial release (`0.1.0`)

### Features

-   Management of destinations through a REST'ish API
-   Permanent/temporary redirects; permanent redirect can not be changed after creation
-   Add notes to destinations to keep track of where destinations are being used
-   Track all hits on destinations, with user agent and ip addres (if possible)
-   Audit log for all creative/destructive management actions
