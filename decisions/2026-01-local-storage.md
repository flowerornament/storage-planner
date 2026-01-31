# Local Storage Decision - January 2026

## Context

Replacing Synology DS224+ NAS (2×8TB RAID1, 3.8TB used) with Mac mini M4-based storage hub.

**Problems with current NAS:**
- Too loud (32dB, home office limit is 30dB)
- Too slow (ARM CPU, HDD speeds)
- HyperBackup to cloud is painfully slow

**Requirements:**
- 8TB local storage (matches NAS capacity)
- Silent operation
- $1,000 hardware budget
- Offsite redundancy sufficient (no local RAID needed)

## Options Evaluated

### Option A: SATA (OWC Mercury Elite Pro Dual Mini)

| Component | Price |
|-----------|-------|
| OWC Mercury Elite Pro Dual Mini Kit | $75 |
| 2× Samsung 870 EVO 4TB (eBay used) | $580-720 |
| **Total** | **$655-795** |

**Specs:**
- Speed: 560 MB/s (SATA limit, 5Gbps via TB on Apple Silicon)
- Bus-powered (no power brick)
- Silent (no fan)
- No expansion

**Topology file:** `topologies/mac-mini-hub-sata.yaml`

### Option B: NVMe (OWC Express 4M2)

| Component | Price |
|-----------|-------|
| OWC Express 4M2 (enclosure) | $240 |
| 2× Lexar NM790 4TB (new) | $840 |
| **Total** | **$1,080** |

**Specs:**
- Speed: 3,200 MB/s (USB4 40Gbps)
- Expandable to 32TB (4 slots, 2 used)
- New drives with 5-year warranty
- Smart fan (near-silent)
- Requires power adapter

**Topology file:** `topologies/mac-mini-hub-nvme.yaml`

## Analysis

Both options pass all redundancy requirements when analyzed with `sp analyze redundancy`.

| Factor | SATA | NVMe |
|--------|------|------|
| Cost | $655-795 | $1,080 |
| Speed | 560 MB/s | 3,200 MB/s |
| Expansion | None | 2 more slots |
| Power | Bus-powered | Adapter required |
| Warranty | Used (none) | 5 years |

**Speed impact for backup workload:**
- Resilio sync: Network-limited (10Gbps LAN) - no difference
- Offsite backup: WAN-limited (500Mbps) - no difference
- Initial migration: NVMe ~6× faster (20 min vs 2 hours for 3.8TB)
- Time Machine: Both adequate

## Decision

**TBD** - Pending final decision.

The NVMe option is ~$300-400 more but provides:
- 6× faster speeds
- Room to grow to 32TB without replacing hardware
- New drives with warranty
- Modern USB4 interface

The SATA option is cheaper and simpler, adequate for backup-centric workload.

## IronWolf HDDs

Current 2× IronWolf 8TB drives can be:
- Sold for ~$160-280 (offsets NVMe premium)
- Kept as monthly cold backup rotation
- Used in a separate enclosure for occasional access

## Migration Plan

1. Purchase hardware
2. Format drives
3. rsync from NAS to new storage (3.8TB, 10-12 hours over LAN)
4. Configure Resilio Sync (MacBook ↔ Mac mini)
5. Configure Time Machine to new storage
6. Verify offsite sync working (borg/rsync to EU server)
7. Run parallel with NAS for 1 week
8. Decommission NAS

## References

- [OWC Mercury Elite Pro Dual Mini](https://eshop.macsales.com/item/OWC/MEMDC2KIT/)
- [OWC Express 4M2](https://eshop.macsales.com/shop/owc-express-4m2)
- SSD pricing volatile due to NAND shortage (Jan 2026)
