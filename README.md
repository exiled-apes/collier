
# collier

## About

This is a quick and dirty toolset for mining NFT crap into a sqlite db where you can make more interesting queries.

These are the tables it currently produces:

```bash
sqlite3 collier.db 'select * from creators limit 1; select * from metadata limit 1; select * from holders limit 1' --header
creator_address|metadata_address
6zqaiXMg3pbJhcYhNskhpCtsKhMeg1egRmkhnAERoxv8|Da3ikeffBUo9G7cvmRjsZ5CC3ZoL7cSDnbmhYEydLBTH
metadata_address|mint_address
Da3ikeffBUo9G7cvmRjsZ5CC3ZoL7cSDnbmhYEydLBTH|592TLcDskgFDZcuSchnMai8XLK8ifYQRZBteeDfobi4X
mint_address|holder_address
2AiDAAnVbx1Ge7xqQsA1Fg7WA4pv5oY6kBdRWg8rpe3h|2ZN4K3SnFXCJoqyT9XNVsfGtLkbygnWWDgqUUqzzYcJh
```

This is a subset of a bunch of other one off tools I've created for mining different collections.

I'll probably keep expanding the common denominator stuff as time permits.

I'll probably ignore contributions, bug reports, etc.

## Usage

### Mine metadata by first creator address

```bash
$ cargo run -q -- -r https://history.genesysgo.net mine-metadata --creator-address=6zqaiXMg3pbJhcYhNskhpCtsKhMeg1egRmkhnAERoxv8
```

Notes:
- typically the candy machine
- this code could be stricter by
-- ensuring that address is a candy machine
-- ensuring the creator signed the NFT and has zero shares

### Mine holders by first creator address

```bash
$ cargo run -q -- -r https://ssc-dao.genesysgo.net mine-holders --creator-address=6zqaiXMg3pbJhcYhNskhpCtsKhMeg1egRmkhnAERoxv8
```