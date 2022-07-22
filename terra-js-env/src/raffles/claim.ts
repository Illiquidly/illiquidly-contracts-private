import { Address } from '../terra_utils';
import { env, add_uploaded_token, add_contract } from '../env_helper';

/// Here we want to upload the p2p contract and add the fee contract
async function main() {
  
  let anyOne = new Address(env['mnemonics'][1]);
  let raffle_contract = anyOne.getContract(env.contracts.raffle);

  let response = await raffle_contract.execute.claim_nft({
    raffle_id: 0,
  })
  console.log(response)
}

main()
  .then((resp) => {})
  .catch((err) => {
    console.log(err);
  });
