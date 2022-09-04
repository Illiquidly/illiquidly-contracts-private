import { Address } from '../terra_utils';
import { env, add_uploaded_token, add_contract } from '../env_helper';
import {MsgMigrateContract, MsgUpdateContractAdmin } from "@terra-money/terra.js";
import * as fs from 'fs';

/// Here we want to upload the p2p contract and add the fee contract
async function main() {
  // Getting a handler for the current address
  let handler = new Address(env['mnemonics'][0]);
  console.log(handler.getAddress());
  // Uploading the contract code
  let p2p = handler.getContract(env.contracts.p2p)

  // Add fee contract to the p2p flow
  let response = await p2p.execute.set_new_fee_contract({
    fee_contract: env.contracts.fee
  });
  console.log(response);

}

main()
  .then((resp) => {
    console.log(resp);
  })
  .catch((err) => {
    console.log(err);
  });
