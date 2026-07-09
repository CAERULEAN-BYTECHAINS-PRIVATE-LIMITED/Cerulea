# Cerulea — Dashboard Architecture

Dashboard is a control surface for operating deployments.

It is not a simple landing page.

## Core sections

### Projects
- list of all projects
- lifecycle state:
  - Draft
  - Building
  - Running
  - Deployed
  - Failed
  - Archived

### Deployments
- history per project
- timestamps
- environment
- logs
- status

### API Keys
- per project
- masked
- revoke
- regenerate
- scope visibility

### Operations monitoring

Must include:

- chain health
- node status
- validator status
- RPC availability
- runtime metrics

## Security

- API keys masked
- revoke confirmation
- activity logs (future)
