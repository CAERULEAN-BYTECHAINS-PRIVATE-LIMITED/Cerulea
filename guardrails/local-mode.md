# Local mode

The build runs on one machine with no third-party account. Several
subsystems normally depend on an external service. This document settles
how each behaves locally, so the no-placeholder rule and the no-credential
reality stop conflicting.

## The principle

A local test double is real software with real behaviour. A placeholder is
a stub returning a fixed value. Cerulea builds against test doubles and
never against placeholders.

Every integration ships two drivers behind one interface. The live driver
speaks to the real service. The local driver implements the same contract
against a local process. The driver is selected by an environment value at
startup. Both drivers are complete, tested and shipped.

## Selection

```
CERULEA_MODE=local      the default for this build
CERULEA_MODE=live       selected when real credentials are present
```

A missing credential in live mode fails at startup with a message naming
the missing value. A missing credential never silently degrades to local.

## The seven integrations

1. **Payments.** The live driver speaks to the payment provider. The local
   driver implements the same interface with an in-process ledger, issuing
   subscription records, invoices and webhook events on the same schedule.
   Every billing path is exercised locally.

2. **Email.** The live driver speaks to the sending provider. The local
   driver writes each message to `local/mail/` as a file and serves an
   inbox view at a local address. Recovery and invitation flows are tested
   end to end.

3. **Certificates.** The live driver requests a certificate from the
   issuing authority. The local driver issues one from a local authority
   created at first run, and trusts that authority in the local browser
   profile. The certificate path is real.

4. **Domain names.** The live driver updates the name service. The local
   driver writes entries into the system hosts file for a reserved local
   suffix, so custom domain attachment is testable.

5. **Object storage.** The live driver speaks to the storage service. The
   local driver uses a directory under `local/objects/` with the same
   interface, including signed access links with expiry.

6. **Content delivery.** The live driver publishes to the delivery
   network. The local driver serves from a local process with the same
   caching headers.

7. **Bridge target networks.** The live driver speaks to a real node on
   each target network. The local driver runs a local development node for
   each target where one exists, and a recorded fixture replay where a
   local node is impractical. Record which targets use which, in the
   decision log.

## What stays out of the repository

No credential, no key and no token is ever committed. Local mode generates
what it needs at first run and writes it under `local/`, which the ignore
list excludes.

Live credentials live in a `.env` file created by hand. The repository
carries `.env.example` listing every required name with no value.

## Acceptance

Every integration carries three tests.

1. The local driver satisfies the interface contract.
2. The live driver satisfies the same contract against a recorded
   transcript.
3. Startup in live mode with a missing credential fails with a message
   naming the missing value.

A task touching an integration is done when all three pass.
