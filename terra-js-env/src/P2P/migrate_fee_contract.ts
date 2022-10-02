import { Address } from '../terra_utils';
import { env } from '../env_helper';
import {MsgMigrateContract } from "@terra-money/terra.js";

/// Here we want to upload the p2p contract and add the fee contract
async function main() {
  // Getting a handler for the current address
  let handler = new Address(env['mnemonics'][0]);
  console.log(handler.getAddress());
  // Uploading the contract code
  let fee_codeId: string[] = await handler.uploadContract(
    '../artifacts/fee_contract.wasm'
  );

  // Migrate the P2P contract
  
  let fee = handler.getContract(env.contracts.fee);
  let migrate_msg = new MsgMigrateContract(handler.getAddress(), fee.address, +fee_codeId[0], {})
  await handler.post([migrate_msg]);
  console.log("migrated the Fee contract");
}

main()
  .then((resp) => {
    console.log(resp);
  })
  .catch((err) => {
    console.log(err);
  });
