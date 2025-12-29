# Gotchas & Pitfalls

Things to watch out for in this codebase.

## [2025-12-29 14:27]
Disk space constraint: The project has limited disk space (141MB available on /Volumes/Work). Full cargo builds may fail with 'No space left on device' error.

_Context: Building this project requires substantial disk space (~2GB+). The target directory should be cleaned if disk space runs low._
