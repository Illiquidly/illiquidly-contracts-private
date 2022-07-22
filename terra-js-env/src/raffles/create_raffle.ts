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

  // we prepare the nft contract : 

  let cw721_tokens = env['cw721'];
  let cw721_token_names = Object.keys(cw721_tokens);
  let nft = handler.getContract(cw721_tokens[cw721_token_names[0]]);
  // First we approve the contract for the NFT

  let token_id = (await nft.query.tokens({owner: handler.getAddress()})).tokens[0]

  let response = await nft.execute.approve({
    spender: raffle_contract.address,
    token_id
  })
  console.log(response)

  response = await raffle_contract.execute.create_raffle({
    asset: {
      cw721_coin:{
        address: nft.address,
        token_id
      }
    },
    raffle_ticket_price: {
      coin:{
        denom: "uluna",
        amount: "476"
      }
    },
    raffle_duration: 30

  })
  console.log(response)
}

main()
  .then((resp) => {})
  .catch((err) => {
    console.log(err);
  });
