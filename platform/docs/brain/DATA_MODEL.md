# Cerulea — Data Model

Cerulea supports multi-tenant structure.

## Entities

### Organization
- id
- name
- owner
- billing

### Team
- id
- org_id
- roles

### User
- id
- org_id
- role
- permissions

### Project
- id
- org_id
- name
- pathway type
- status

### Deployment
- id
- project_id
- environment
- status
- runtime config

### API Key
- id
- project_id
- scope
- created_at
- last_used
