# Every request declares its client version

Every client request carries a version header from day one, for example `Agent-Inc-Client: mac/1.4.2 (build 812; api 1)`; every response carries the server version. A configured minimum client version returns one typed upgrade-required error, and an incompatible client returns the reciprocal server-too-old error. The generated client handles both centrally, and the app shows one generic "Update to continue" screen.
