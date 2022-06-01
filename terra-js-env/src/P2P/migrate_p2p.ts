import { Address } from '../terra_utils';
import { env } from '../env_helper';
import {MsgMigrateContract } from "@terra-money/terra.js";

/// Here we want to upload the p2p contract and add the fee contract
async function main() {
  // Getting a handler for the current address
  let handler = new Address(env['mnemonics'][0]);
  console.log(handler.getAddress());
  // Uploading the contract code
  let p2p_codeId: string[] = await handler.uploadContract(
    '../artifacts/p2p_trading.wasm'
  );

  // Migrate the P2P contract
  
  let p2p = handler.getContract(env.contracts.p2p);
  let migrate_msg = new MsgMigrateContract(handler.getAddress(), p2p.address, +p2p_codeId[0], {})
  await handler.post([migrate_msg]);
  console.log("migrated the P2P contract");
}

main()
  .then((resp) => {
    console.log(resp);
  })
  .catch((err) => {
    console.log(err);
  });
