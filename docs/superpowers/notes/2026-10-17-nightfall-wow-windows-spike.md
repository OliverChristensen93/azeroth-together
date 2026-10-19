# Nightfall: can WoW's own windows be fed without a server? (spike)

Date: 2026-10-19. Timebox: 3 hours; used: about 1.5. Plan: docs/superpowers/plans/2026-10-17-nightfall-inn.md, Task 12.

## Answer

Yes for gossip and merchant, shown working; very likely yes for quest; bank and bags are possible but
are a different order of work. Gossip and merchant need no packet and no server: fill the window's
resource from a system of ours (what the `SMSG_` handler would do) and WoW's own frame draws, with
the real art, the NPC's name and portrait (merchant), and the merchant's row layout. The window's
buttons land as intents in `UiScript`, and we can read them before the net sees them. What a server
would have confirmed (a purchase, an item moving in a bag) we must answer ourselves, as a pretend
server: the quest log, the bags and the bank are descriptor fields plus item objects, and the drain
code there waits for the server's updates. The spike code was thrown away; none of it is committed.

## Gossip (GossipFrame)

- Data and feed: `ui_gossip::GossipState` (npc guid, text id, options with wire `index`/`icon`/`coded`,
  quest rows, and a cache of NPC-text records by id). `feed_gossip` (in `UiFeed`) turns it into a
  `GossipMenu` for the VM and fires `GOSSIP_SHOW` (with the NPC's name as arg1, from `NameCache`) and
  `GOSSIP_CLOSED`. Opened by `net::on_gossip_menu` (`SMSG_GOSSIP_MESSAGE`) then `on_npc_greeting`
  (`SMSG_NPC_TEXT_UPDATE`). The menu is held closed until its greeting is drawn.
- What opened it here: `remember_record(id, [NpcTextBlock])` then `open_menu(guid, id, options, [], 0)`
  (both `pub(crate)`). With the record cached, `open_menu` returns false (no `NpcTextQuery` owed) and draws
  the greeting at once. The NPC guid is an inn NPC's guid (a player-kind guid, `0x100 + index`, already in
  `GuidIndex` with a `Transform`). Nothing else was faked: `InteractNpc` is not set by hand, it is derived
  from the open resources (`ui_session::feed_interact_npc`), so it follows by itself.
- Things that bit:
  - The range guard (`close_npc_session_out_of_range::<GossipState>`) closes any session whose NPC is
    farther than the leash, every frame. Our NPCs have player-kind guids, so the leash is 14 yd
    (`PLAYER_LEASH_SQ`), not the 5.56 yd service gate. From the hero's spawn the banker is 14.1 yd away
    and its window closed in one frame (the log shows `gossip.npc` going `None`); the trader, 11.7 yd,
    stayed open (so the shot used the trader as the gossip NPC). A real use must either keep the hero in
    reach (our `walk_away` rule is 6 yd, so it would) or make the NPCs creature-kind guids (5.56 yd leash).
    `NIGHTFALL_PANEL`'s "stays open wherever the hero stands" cannot be kept with these windows.
  - The title and portrait of the gossip frame showed blank and a book icon in the shot, while the
    merchant's showed the NPC's name and face (same NPC, same guid). Not run down in the timebox.
- What its buttons send, and whether we can take it before the net: `drain_gossip` (after `UiInput`)
  reads `script.take_gossip_selects()` (1-based position) and sends `ClientCommand::GossipSelectOption
  { guid, option: wire index }` on `NetCommands`; a quest row sends `QuestgiverQuery`/`QuestgiverComplete`;
  Goodbye (`CloseGossip`) is client-side (`take_gossip_close`, clears the state, no packet). In the spike
  `SelectGossipOption(1)` run through `UiScript::eval` queued `[1]`, readable with `take_gossip_selects()`
  before any drain. `NetCommands` has no reader we can attach to (its receiver is the IO thread's), so the
  intercept is "take the intents first", as `take_action_uses` is taken in the combat plan. The click was
  made from Lua, not with a mouse: a real mouse click on the frame was not tried.
- Shot: target/spike-gossip.png (WoW's own gossip window on the left, parchment with the greeting
  "Your things are safe with me, traveller." and two options, "Show me my things" and "Goodbye", and its
  Goodbye button; our own inn bar stays up top).

## Merchant (MerchantFrame)

- Data and feed: `ui_merchant::MerchantOpen { vendor, items: Vec<VendorItem> }`; `feed_merchant` builds
  `MerchantItem` rows (icon from `ItemDisplays` by the row's `display_id`; name, quality and stats from an
  `Items` template; price in copper; stock, `-1` for unlimited) plus the buyback and repair rows and the
  purse from the player's descriptor, and fires `MERCHANT_SHOW`/`MERCHANT_CLOSED`. Opened by `net::on_inventory`
  (`SMSG_LIST_INVENTORY`).
- What opened it here: `MerchantOpen::open(trader_guid, rows)`, with one `VendorItem` per entry of the hub's
  `trader.stock` (`slot` 1.., `entry` a stand-in 0x7100_0000 + n, `current_count` `0xFFFF_FFFF`,
  `buy_count` 1, `price` in copper), and for each entry a stand-in `ItemInfo` filed with
  `Items::insert_template(entry, Some(info))` (name, `display_info_id`, quality, `stackable`; the rest
  zero, as `dress.rs` does for outfits). Without a template the row has no name and no tooltip. Our items
  carry an icon *name* (`INV_Misc_Food_11`), not a display id, so the spike mapped name to display id by
  scanning `ItemDisplays::catalog` (all six trader rows matched). The shot shows all six rows with the
  right icons, names and prices, the NPC's name ("Haldor Coppermug") and portrait, the Merchant and Buyback
  tabs, and the backpack, which WoW's UI opened by itself. The purse reads 0: it comes from the player's
  money descriptor field, which the inn does not set.
- What a Buy click writes and whether we can take it first: `drain_merchant` reads
  `take_merchant_buys()` (1-based row, quantity), turns the row into the entry (`entry_at`) and sends
  `ClientCommand::BuyItem { vendor, entry, count }` (`CMSG_BUY_ITEM`); a sell (a bag item dropped on the
  window) sends `SellItem { vendor, item_guid, count }`; a buyback `BuybackItem`; Repair All `RepairItem`.
  In the spike `BuyMerchantItem(1, 1)` (again from Lua) queued `[(1, 1)]` for `take_merchant_buys()`, so a
  buy can be taken before the net, and mapped by us from row to the trader's stock row and price. The reply
  a real server gives (`SMSG_BUY_ITEM`'s new stock, an `SMSG_UPDATE_OBJECT` creating the bought item, the
  money field dropping) is ours to give: `MerchantOpen::update_stock`, `MerchantErrors` (a refusal's message
  line) and the descriptor fields. Selling needs the item as an object in a bag slot (see bags below).
- Shot: target/spike-merchant.png.

## Quest (QuestFrame)

Not built (read only). The window is `ui_quest::QuestGiver { npc, view: Option<QuestView> }`, where
`QuestView` is the packet that opened it: `Greeting(QuestGiverList)` (npc, greeting, rows `QuestListEntry
{ quest_id, icon (dialog status), level, title }`), `Detail(QuestDetails)` (title, details text, objectives
text, `choices` and `rewards` as `QuestRewardItem { item_id, count, display_id }`, money, reward spell),
`Progress(QuestRequestItems)` and `Reward(QuestOfferReward)`. `open(npc, view)` is the same shape as the
other two, and reward items need `Items` templates like the merchant's. Drains send
`QuestgiverHello`/`Query`/`Accept`/`RequestReward`/`ChooseReward`/`Complete` (all intents we can take
first). Estimate: the window itself is about as easy as gossip (half a day, with the `QuestView` values built
by a small adapter from our contracts). The quest *log* (`ui_quest_log`) and the tracker read the player's
descriptor quest slots and need the player's quest state as fields; that is a day, and only if we want WoW's
log too. Our contracts have no accept/progress/reward three-panel shape, so a mapping must be chosen.

## Bank (BankFrame) and bags (ContainerFrame)

Not built (read only). `ui_bank::BankOpen { banker }` is trivial (open it, and the frame appears; its only
buttons are Buy Slot, `BuyBankSlot`, and close). But the bank's contents are not in it: the 24 vault slots
are the player's descriptor bank-slot fields (container `-1`) and bank bags `5..=10` hold item *objects*;
both are read by `ui_items` from the self player's `ObjectStore` plus each item's own object (entry, stack
count, durability) in the `GuidIndex`, with templates in `Items`. A move (right-click, a drag, a swap)
sends `AutoStoreBankItem`/`AutoBankItem`/`SwapItem`/`SwapInvItem`/`SplitItem` and *waits for the server*:
`PendingItemOps` locks the slots until the object updates arrive. So feeding the bank means acting as a
server for item moves: keep our stash as descriptor fields and item objects, take each item command, move
the stash, and write the fields back, including lock and refusal behaviour. That is also the backpack
(WoW's own opened by itself in the merchant shot, empty): our own backpack and stash views would be
replaced, and the stash's rules (read-only stash, the stash lock, weight, bag slots by items) would have
to be re-expressed as slots and bag sizes. Estimate: several days, the highest risk of the four, and the
least gain, since our panels already do it and the spec's weight and stack rules have no place in WoW's
bag UI.

## Recommendation

Keep our own panels for the shipped path. If a look-and-feel win is wanted, switch in this order, each its
own plan: (1) gossip for NPC greetings and choices (cheapest); (2) the trader as WoW's merchant frame (the
spike's code was about 40 lines; it needs the stand-in templates for the stock, an icon-name to display-id
lookup, the money field for the purse, and our own handling of a buy); (3) quest; leave the bank and bags
on our panels.

Engine-side seams a switch needs, all small (new files or visibility, no upstream logic changed): (a) a way
to take an intent before its drain: either make the drain systems `pub(crate)` so ours can be ordered
`.before(drain_gossip)`, or a `UiDrain` system set that all the drains join (the drains are `.after(UiInput)`
and private, so an intercept of ours would be unordered against them); (b) the range guard: a window for an
inn NPC must close by our rule (6 yd) or the NPC must be a creature-kind guid; the guard runs for any open
`NpcSession`; (c) a feed that sets the player's money field (and bag descriptor fields, for sell) in the
self player's `ObjectStore`, for the purse. The seams the spike used, `GossipState::remember_record` and
`open_menu`, `MerchantOpen::open`, `Items::insert_template`, are already `pub(crate)`.
