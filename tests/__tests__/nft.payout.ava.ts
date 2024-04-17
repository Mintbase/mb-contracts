import avaTest from "ava";
import { NEAR, mintingDeposit } from "./utils/index.js";
import { MB_VERSION, setup } from "./setup.js";
import { NearAccount } from "near-workspaces";

const test = setup(avaTest);

const mint = async ({
  alice,
  store,
  royalty_args,
  split_owners,
}: {
  alice: NearAccount;
  store: NearAccount;
  royalty_args?: { split_between: Record<string, number>; percentage: number };
  split_owners?: Record<string, number>;
}): Promise<string> => {
  if (MB_VERSION == "v1") {
    await alice.call(
      store,
      "nft_batch_mint",
      {
        owner_id: alice.accountId,
        metadata: {},
        num_to_mint: 1,
        royalty_args,
        split_owners,
      },
      { attachedDeposit: mintingDeposit({ n_tokens: 1, n_splits: 20 }) }
    );
    return "0";
  }

  await alice.call(
    store,
    "create_metadata",
    { metadata: {}, price: NEAR(0.01), royalty_args },
    { attachedDeposit: NEAR(0.1) }
  );

  await alice.call(
    store,
    "deposit_storage",
    { metadata_id: "0" },
    { attachedDeposit: NEAR(0.05) }
  );

  await alice.call(
    store,
    "mint_on_metadata",
    {
      metadata_id: "0",
      num_to_mint: 3,
      owner_id: alice.accountId,
    },
    { attachedDeposit: NEAR(0.03) }
  );

  if (split_owners) {
    await alice.call(
      store,
      "set_split_owners",
      {
        token_ids: ["0:0"],
        split_between: split_owners,
      },
      { attachedDeposit: NEAR(0.02) }
    );
  }

  return "0:0";
};

test("payout::splits", async (test) => {
  // slightly different paradigm, this test doesn't make sense for v2
  if (MB_VERSION === "v2") {
    test.pass();
    return;
  }
  const { alice, store } = test.context.accounts;

  const split_owners = (() => {
    const o: Record<string, number> = {};
    o["a.near"] = 6000;
    o["b.near"] = 4000;
    return o;
  })();

  const tokenId = await mint({ alice, store, split_owners });

  const payout = (() => {
    const p: Record<string, string> = {};
    p["a.near"] = "6000000000000000";
    p["b.near"] = "4000000000000000";
    return p;
  })();
  test.deepEqual(
    await store.view("nft_payout", {
      token_id: tokenId,
      balance: "10000000000000000",
    }),
    { payout }
  );
});

test("payout::royalties", async (test) => {
  // tested via v2 minting
  if (MB_VERSION === "v2") {
    test.pass();
    return;
  }

  const { alice, store } = test.context.accounts;

  const split_between = (() => {
    const o: Record<string, number> = {};
    o["a.near"] = 5000;
    o["b.near"] = 5000;
    return o;
  })();

  const tokenId = await mint({
    alice,
    store,
    royalty_args: { split_between, percentage: 4000 },
  });

  const payout = (() => {
    const p: Record<string, string> = {};
    p["a.near"] = "2000000000000000";
    p["b.near"] = "2000000000000000";
    p[alice.accountId] = "6000000000000000";
    return p;
  })();
  test.deepEqual(
    await store.view("nft_payout", {
      token_id: tokenId,
      balance: "10000000000000000",
    }),
    { payout }
  );
});

test("payout::royalties_splits", async (test) => {
  const { alice, store } = test.context.accounts;

  const split_between = (() => {
    const o: Record<string, number> = {};
    o["a.near"] = 7500;
    o["b.near"] = 2500;
    return o;
  })();

  const split_owners = (() => {
    const o: Record<string, number> = {};
    o["c.near"] = 7500;
    o["d.near"] = 2500;
    return o;
  })();

  const tokenId = await mint({
    alice,
    store,
    royalty_args: { split_between, percentage: 2000 },
    split_owners,
  });

  const payout = (() => {
    const p: Record<string, string> = {};
    p["a.near"] = "1500000000000000";
    p["b.near"] = "500000000000000";
    p["c.near"] = "6000000000000000";
    p["d.near"] = "2000000000000000";
    return p;
  })();
  test.deepEqual(
    await store.view("nft_payout", {
      token_id: tokenId,
      balance: "10000000000000000",
    }),
    { payout }
  );
});

test("payout::low_balance", async (test) => {
  const { alice, store } = test.context.accounts;

  const tokenId = await mint({
    alice,
    store,
    split_owners: { "a.near": 6000, "b.near": 4000 },
  });

  test.deepEqual(
    await store.view("nft_payout", {
      token_id: tokenId,
      balance: "10000",
    }),
    { payout: { "a.near": "6000", "b.near": "4000" } }
  );
});

test("payout::max_len", async (test) => {
  const { alice, store } = test.context.accounts;

  const split_owners = (() => {
    const o: Record<string, number> = {};
    o["a.near"] = 1000;
    o["b.near"] = 950;
    o["c.near"] = 900;
    o["d.near"] = 850;
    o["e.near"] = 800;
    o["f.near"] = 750;
    o["g.near"] = 700;
    o["h.near"] = 650;
    o["i.near"] = 600;
    o["j.near"] = 550;
    o["k.near"] = 500;
    o["l.near"] = 450;
    o["m.near"] = 400;
    o["n.near"] = 350;
    o["o.near"] = 300;
    o["p.near"] = 250;
    return o;
  })();

  const tokenId = await mint({
    alice,
    store,
    split_owners,
  });

  // FIXME: should work with lower number
  const payout = (() => {
    const p: Record<string, string> = {};
    p["a.near"] = "1000000000000000";
    p["b.near"] = "950000000000000";
    p["c.near"] = "900000000000000";
    p["d.near"] = "850000000000000";
    p["e.near"] = "800000000000000";
    p["f.near"] = "750000000000000";
    p["g.near"] = "700000000000000";
    p["h.near"] = "650000000000000";
    p["i.near"] = "600000000000000";
    p["j.near"] = "550000000000000";
    return p;
  })();
  test.deepEqual(
    await store.view("nft_payout", {
      token_id: tokenId,
      balance: "10000000000000000",
      max_len_payout: 10,
    }),
    { payout }
  );
});
