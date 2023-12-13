import { TestFn } from "ava";
import { Worker, NearAccount } from "near-workspaces";
import * as nearAPI from "near-api-js";
import {
  DEPLOY_STORE_RENT,
  DEPLOY_STORE_GAS,
  mintingDeposit,
  NEAR,
} from "./utils/balances.js";
import { getEvent } from "./utils/events.js";

export const MB_VERSION = process.env.MB_VERSION || "v1";
export const CHANGE_SETTING_VERSION = {
  v1: "0.1.0",
  v2: "0.2.0",
}[MB_VERSION];

const createSubaccount = async (
  root: NearAccount,
  name: string,
  { initialBalanceNear }: { initialBalanceNear: string }
): Promise<NearAccount> =>
  root.createAccount(`${name}.${root.accountId}`, {
    initialBalance: nearAPI.utils.format.parseNearAmount(
      initialBalanceNear
    ) as string,
  });

export const createAndDeploy = async (
  root: NearAccount,
  name: string,
  args: {
    initialBalanceNear: string;
    codePath: string;
    initMethod: string;
    initArgs: any;
  }
): Promise<NearAccount> => {
  const { codePath, initMethod, initArgs } = args;
  const account = await createSubaccount(root, name, args);
  await account.deploy(codePath);
  await account.call(account, initMethod, initArgs);
  return account;
};

export const deployStore = async ({
  factory,
  owner,
  name,
}: {
  factory: NearAccount;
  owner: NearAccount;
  name: string;
}): Promise<NearAccount> => {
  const res = await owner.callRaw(
    factory,
    "create_store",
    {
      owner_id: owner.accountId,
      metadata: {
        spec: "nft-1.0.0",
        name,
        symbol: "ALICE",
      },
    },
    { attachedDeposit: DEPLOY_STORE_RENT, gas: DEPLOY_STORE_GAS }
  );
  return factory.getAccount(`${name}.${factory.accountId}`);
};

type TestContext = {
  worker: Worker;
  accounts: Record<string, NearAccount>;
};

export const setup = (test: TestFn): TestFn<TestContext> => {
  test.beforeEach(async (t) => {
    const worker = await Worker.init();
    const root = worker.rootAccount;
    const alice = await createSubaccount(root, "alice", {
      initialBalanceNear: "20",
    });
    const bob = await createSubaccount(root, "bob", {
      initialBalanceNear: "20",
    });
    const carol = await createSubaccount(root, "carol", {
      initialBalanceNear: "20",
    });
    const dave = await createSubaccount(root, "dave", {
      initialBalanceNear: "20",
    });

    const factory = await createAndDeploy(root, "factory", {
      initialBalanceNear: "10",
      codePath: `../wasm/factory-${MB_VERSION}.wasm`,
      initMethod: "new",
      initArgs: {},
    });
    // const store = await createAndDeploy(root, "store", {
    //   initialBalanceNear: "10",
    //   codePath: "../wasm/store.wasm",
    //   initMethod: "new",
    //   initArgs: {
    //     owner_id: root,
    //     metadata: {
    //       spec: "nft-1.0.0",
    //       name: `store.${root}`,
    //       symbol: "STORE",
    //     },
    //   },
    // });
    const oldMarket = await createAndDeploy(root, "market", {
      initialBalanceNear: "20",
      codePath: "../wasm/legacy-market.wasm",
      initMethod: "new",
      initArgs: { init_allowlist: [] },
    });
    const newMarket = await createAndDeploy(oldMarket, "simple", {
      initialBalanceNear: "10",
      codePath: "../wasm/interop-market.wasm",
      initMethod: "init",
      initArgs: {
        owner: root,
        mintbase_cut: 5000,
        fallback_cut: 250,
        listing_lock_seconds: "0",
      },
    });

    const store = await deployStore({ owner: alice, factory, name: "alice" });

    (t.context as TestContext).worker = worker;
    (t.context as TestContext).accounts = {
      root,
      alice,
      bob,
      carol,
      dave,
      factory,
      store,
      oldMarket,
      newMarket,
    };
  });

  test.afterEach(async (t) => {
    await (t.context as TestContext).worker.tearDown().catch((e) => {
      console.log("Failed to tear down the worker:", e);
    });
  });

  return test as TestFn<TestContext>;
};
export default setup;

export const mint = async ({
  minter,
  store,
  n,
}: {
  minter: NearAccount;
  store: NearAccount;
  n: number;
}): Promise<string[]> => {
  if (MB_VERSION === "v1") {
    const mintCall = await minter.callRaw(
      store,
      "nft_batch_mint",
      { owner_id: minter, metadata: {}, num_to_mint: n },
      { attachedDeposit: mintingDeposit({ n_tokens: n }) }
    );
    return getEvent(mintCall.result.receipts_outcome[0].outcome.logs[0]).data[0]
      .token_ids;
  } else {
    // Create metadata
    const createMetadataCall = await minter.callRaw(
      store,
      "create_metadata",
      { metadata: {}, price: NEAR(1) },
      { attachedDeposit: mintingDeposit({ n_tokens: n }) } // TODO: should be a different deposit
    );
    const metadata_id = getEvent(
      createMetadataCall.result.receipts_outcome[0].outcome.logs[0]
    ).data.metadata_id;
    // mint on the metadata
    const mintCall = await minter.callRaw(
      store,
      "mint_on_metadata",
      { metadata_id, owner_id: minter, num_to_mint: n },
      { attachedDeposit: NEAR(1.1) } // TODO: should be a different deposit
    );
    return getEvent(mintCall.result.receipts_outcome[0].outcome.logs[0]).data[0]
      .token_ids;
  }
};
