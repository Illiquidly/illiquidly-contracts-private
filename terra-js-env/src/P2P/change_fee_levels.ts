import { Address } from '../terra_utils';
import { env, add_uploaded_token, add_contract } from '../env_helper';
import {MsgMigrateContract, MsgUpdateContractAdmin } from "@terra-money/terra.js";
import * as fs from 'fs';

/// Here we want to upload the p2p contract and add the fee contract
async function main() {
  // Getting a handler for the current address
  let handler = new Address(env['mnemonics'][0]);
  // Uploading the contract code
  let feeContract = handler.getContract(env.contracts.fee)

  // Add fee contract to the p2p flow
  	/*
	let response = await feeContract.execute.update_fee_rates({
		first_teer_rate: '10',
		second_teer_rate: '5',
		third_teer_rate: '2',
	});
	*/
	let response = await feeContract.query.fee_rates({
		first_teer_rate: '10',
		second_teer_rate: '5',
		third_teer_rate: '2',
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
