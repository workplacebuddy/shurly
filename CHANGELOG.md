# Changelog

## Shurly

> Shurly, this is a URL shortener with API management

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
