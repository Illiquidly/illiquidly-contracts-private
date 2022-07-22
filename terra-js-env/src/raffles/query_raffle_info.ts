import { Address } from '../terra_utils';
import { env, add_uploaded_token, add_contract } from '../env_helper';
import { Numeric } from '@terra-money/terra.js';
function getContractLog(response: any) {
  return response.logs[0].eventsByType.from_contract;
}

/// Here we want to upload the p2p contract and add the fee contract
async function main() {

  let handler = new Address(env['mnemonics'][0]);
  console.log(handler.getAddress())
  let raffle_contract = handler.getContract(env.contracts.raffle);

  // First we approve the contract for the NFT

  let response = await raffle_contract.query.raffle_info({
   raffle_id: 0,
 })
  console.log(response)
}

main()
  .then((resp) => {})
  .catch((err) => {
    console.log(err);
  });
