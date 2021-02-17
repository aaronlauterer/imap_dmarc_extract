# imap_dmarc_extract

This small tool will connect to an imap account and extract all DMARC reports from the mails present in the INBOX folder.

## Parameters

```
imap_dmarc_extract <server> <account> <path>
```

If you need to use a port other than 993, define server in the following way: `mail.mydomain.com:<port>`.


## Building

After cloning the repository, simply run
```
cargo build --release
```
