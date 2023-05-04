import { Gas, BN, NearAccount } from "near-workspaces";
import * as nearWs from "near-workspaces";
import * as nearAPI from "near-api-js";
import { ExecutionContext } from "ava";

// TODO: move from this format to `ava.NEAR.parse`

/**
 * Interprets a float as NEAR and builds the corresponding string.
 * Rounded to closest milliNEAR.
 */
export function NEAR(x: number): nearWs.NEAR {
  return mNEAR(x).mul(new nearWs.NEAR(1e3));
}

/**
 * Interprets a float as milliNEAR and builds the corresponding string.
 * Rounded to closest microNEAR.
 */
export function mNEAR(x: number): nearWs.NEAR {
  return uNEAR(x).mul(new nearWs.NEAR(1e3));
}

/**
 * Interprets a float as microNEAR and builds the corresponding string.
 * Rounded to closest nanoNEAR.
 */
export function uNEAR(x: number): nearWs.NEAR {
  return nNEAR(x).mul(new nearWs.NEAR(1e3));
}

/**
 * Interprets a float as nanoNEAR and builds the corresponding string.
 * Rounded to closest picoNEAR.
 */
export function nNEAR(x: number): nearWs.NEAR {
  return new nearWs.NEAR((x * 1e3).toString() + "0".repeat(12));
}

/**
 * Interprets a float as Teragas and builds the corresponding string.
 * Rounded to closest Gigagas.
 */
export function Tgas(x: number): nearWs.Gas {
  return new nearWs.Gas((x * 1e3).toString() + "0".repeat(9));
}

/**
 * Interprets a float as NEAR and builds the corresponding `BN`.
 * Rounded to closest milliNEAR.
 */
export function NEARbn(x: number): BN {
  return new BN(NEAR(x));
}

/**
 * Interprets a float as milliNEAR and builds the corresponding `BN`.
 * Rounded to closest microNEAR.
 */
export function mNEARbn(x: number): BN {
  return new BN(mNEAR(x));
}

/**
 * Interprets a float as microNEAR and builds the corresponding `BN`.
 * Rounded to closest nanoNEAR.
 */
export function uNEARbn(x: number): BN {
  return new BN(uNEAR(x));
}

/**
 * Interprets a float as nanoNEAR and builds the corresponding `BN`.
 * Rounded to closest picoNEAR.
 */
export function nNEARbn(x: number): BN {
  return new BN(nNEAR(x));
}

/**
 * Interprets a float as Teragas and builds the corresponding `BN`.
 * Rounded to closest Gigagas.
 */
export function Tgasbn(x: number): BN {
  return new BN(Tgas(x));
}

// Conversion methods for interop market tests
export const nearToYocto = nearAPI.utils.format.parseNearAmount;
export const yoctoToNear = nearAPI.utils.format.formatNearAmount;
export const yoctoToBn = (yocto: string): BN => new BN(yocto);
export const bnToYocto = (bn: BN): string => bn.toString();
export const nearToBn = (near: string): BN =>
  yoctoToBn(nearToYocto(near) as string);
export const bnToNear = (bn: BN): string => yoctoToNear(bnToYocto(bn));

/** Maximum possible gas (will be serialized to a u64) */
export const MAX_U64 = new BN("ffffffffffffffff", 16);
/** Gas cost for deploying a store (taken from mintbase-js) */
export const DEPLOY_STORE_GAS = Tgas(200);
/** Storage rent for deploying a store (taken from mintbase-js) */
export const DEPLOY_STORE_RENT = NEAR(3.5);
/** Storage rent for deploying a store (taken from mintbase-js) */

export const mintingDeposit = ({
  n_tokens,
  n_royalties,
  n_splits,
  metadata_bytes,
}: {
  n_tokens: number;
  n_royalties?: number;
  n_splits?: number;
  metadata_bytes?: number;
}): string => {
  //80 bytes * 10e18 NEAR/byte = 0.8e21
  const common_deposit = 0.8;
  // 360 bytes * 10e18 NEAR/byte = 3.6e21
  const token_deposit = 3.6;
  const minting_fee = 1;

  const metadata_deposit = (metadata_bytes || 10000) * 0.001;
  const splits_deposit = (n_splits || 0) * common_deposit;
  const royalties_deposit = (n_royalties || 0) * common_deposit;
  const total =
    metadata_deposit +
    royalties_deposit +
    n_tokens * (token_deposit + splits_deposit + common_deposit) +
    minting_fee;
  return mNEAR(Math.ceil(total)).toString();
};

/**
 * Mostly a wrapper for getting total balance of an account, might change to
 * available balance in the future.
 */
export async function getBalance(account: NearAccount): Promise<nearWs.NEAR> {
  return (await account.balance()).total;
}

// TODO::testing::low: use this function consistently
/** Asserts balance changes for multiple accounts in parallel */
export async function assertBalanceChanges(
  test: ExecutionContext,
  specs: {
    account: NearAccount;
    ref: nearWs.NEAR;
    diff: nearWs.NEAR;
  }[],
  msg: string
) {
  await Promise.all(specs.map((spec) => assertBalanceChange(test, spec, msg)));
}

/**
 * Asserts the change of an account balance w.r.t. an earlier reference amount.
 * The balance is allowed to be 0.05 NEAR below `ref - diff`, which accounts for
 * gas costs that might have been expended.
 */
export async function assertBalanceChange(
  test: ExecutionContext,
  params: {
    account: NearAccount;
    ref: nearWs.NEAR;
    diff: nearWs.NEAR;
    gas?: Gas;
  },
  msg: string
) {
  const now = await getBalance(params.account);
  if (params.gas) {
    const { gas } = params;
    assertBalanceDiffExact(test, { ...params, now, gas }, msg);
  } else {
    const maxGas = NEAR(0.05).toString(); // allow 40 mNEAR of gas costs
    assertBalanceDiffRange(test, { ...params, now, maxGas }, msg);
  }
}

function assertBalanceDiffExact(
  test: ExecutionContext,
  {
    account,
    now,
    ref,
    diff,
    gas,
  }: {
    account: NearAccount;
    now: nearWs.NEAR;
    ref: nearWs.NEAR;
    diff: nearWs.NEAR;
    gas: Gas;
  },
  msg: string
) {
  const nearGas = new nearWs.NEAR(gas.mul(new BN(100e6)).toString());
  const expected = ref.add(diff).sub(nearGas);
  // test.log({
  //   account: account.accountId,
  //   expected: expected.toString(),
  //   now: now.toString(),
  //   ref: ref.toString(),
  //   diff: diff.toString(),
  //   nearGas: nearGas.toString(),
  // });

  test.true(
    now.eq(expected),
    [
      `${msg}: wrong balance for ${account.accountId}`,
      `\texpected: ${expected.toHuman()}`,
      `\tactual:   ${now.toHuman()}`,
    ].join("\n")
  );

  test.fail(
    [
      `${msg}: balance for ${account.accountId}`,
      `\texpected: ${expected.toHuman()}`,
      `\tactual:   ${now.toHuman()}`,
    ].join("\n")
  );
}

// TODO::testing::low: deprecate this (blocked until gas stuff becomes more sound)
function assertBalanceDiffRange(
  test: ExecutionContext,
  {
    account,
    now,
    ref,
    diff,
    maxGas,
  }: {
    account: NearAccount;
    now: nearWs.NEAR;
    ref: nearWs.NEAR;
    diff: nearWs.NEAR;
    maxGas: string;
  },
  msg: string
) {
  // test.log("entering assertBalanceDiffRange");
  const max = ref.add(new BN(diff));
  const min = max.sub(new BN(maxGas));
  // test.log({
  //   account: account.accountId,
  //   now: now.toString(),
  //   ref: ref.toString(),
  //   diff: diff.toString(), // cannot use toHuman on negative diff!
  //   min: min.toString(),
  //   max: max.toString(),
  // });
  test.true(now.lte(max), `${msg}: balance too high for ${account}`);
  test.true(now.gte(min), `${msg}: balance too low for ${account}`);
}

// diff checking from interop market
export const bnInRange = (bn: BN, lower: BN, upper: BN): boolean => {
  return bn.gte(lower) && bn.lt(upper);
};

export const diffCheck = (
  bn: BN,
  ref: BN,
  diff: BN,
  slippage?: BN
): boolean => {
  const upper = ref.add(diff);
  if (slippage) {
    const lower = upper.sub(slippage);
    return bnInRange(bn, lower, upper);
  } else {
    return bn.eq(upper);
  }
};
