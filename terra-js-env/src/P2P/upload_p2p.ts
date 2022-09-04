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
  let p2p_codeId: string[] = await handler.uploadContract(
    '../artifacts/p2p_trading.wasm'
  );
  let fee_codeId: string[] = await handler.uploadContract(
    '../artifacts/fee_contract.wasm'
  );
  let feeDistributor_codeId: string[] = await handler.uploadContract(
    '../artifacts/fee_distributor.wasm'
  );


  // Initialize p2p contract
  let p2pInitMsg = {
    name: 'P2PTrading'
  };

  let p2p = await handler.instantiateContract(+p2p_codeId[0], p2pInitMsg);
  add_contract('p2p', p2p.address);

  console.log('Uploaded the p2p contract');

  // Initialize fee_distributor contract
  let feeDistributorInitMsg = {
    name: 'feeDistributor',
    treasury: handler.getAddress()
  };

  let feeDistributor = await handler.instantiateContract(+feeDistributor_codeId[0],  feeDistributorInitMsg);
  add_contract('fee_distributor', feeDistributor.address);

  console.log('Uploaded the distributor contract');



  // Initialize fee contract
  let feeInitMsg = {
    name: 'FirstFeeContract',
    p2p_contract: p2p.address,
    treasury: handler.getAddress(),
    fee_distributor: feeDistributor.address
  };

  let fee = await handler.instantiateContract(+fee_codeId[0], feeInitMsg);
  add_contract('fee', fee.address);

  console.log('Uploaded the fee contract');

  // Add fee contract to the p2p flow
  let response = await p2p.execute.set_new_fee_contract({
    fee_contract: fee.address
  });
  console.log(response);

  return ['p2p', p2p.address];
}

main()
  .then((resp) => {
    console.log(resp);
  })
  .catch((err) => {
    console.log(err);
  });
