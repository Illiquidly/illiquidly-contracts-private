import { Address } from '../terra_utils';
import { env } from '../env_helper';
import {MsgMigrateContract } from "@terra-money/terra.js";

/// Here we want to upload the p2p contract and add the fee contract
async function main() {
  // Getting a handler for the current address
  let handler = new Address(env['mnemonics'][0]);
  console.log(handler.getAddress());
  // Uploading the contract code
  let raffle_codeId: string[] = await handler.uploadContract(
    '../artifacts/raffles.wasm'
  );

  // Migrate the Raffle contract
  let raffle = handler.getContract(env.contracts.raffle);
  let migrate_msg = new MsgMigrateContract(handler.getAddress(), raffle.address, +raffle_codeId[0], {})
  await handler.post([migrate_msg]);
  console.log("migrated the raffles contract");
}

main()
  .then((resp) => {
    console.log(resp);
  })
  .catch((err) => {
    console.log(err);
  });
