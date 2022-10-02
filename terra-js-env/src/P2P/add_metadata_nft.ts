import { Address } from '../terra_utils';
import { env } from '../env_helper';

function getContractLog(response: any) {
  console.log(response);
  return response.logs[0].eventsByType.from_contract;
}

async function main() {
  // Getting a handler for the current address
  let handler = new Address(env['mnemonics'][0]);

  // Get the token_id


  let cw721_tokens = env['cw721'];
  let cw721_token_names = Object.keys(cw721_tokens);
  let nft_contract = handler.getContract(cw721_tokens[cw721_token_names[1]]);



  console.log(nft_contract.address)
  let token_id_response = await nft_contract.query.tokens({
    owner: handler.getAddress()
  })  

  let p2p = handler.getContract(env.contracts.p2p)

  // We approve the NFT
  await nft_contract.execute.approve({
    token_id: token_id_response.tokens[0],
    spender: p2p.address
  });

  // We try to add trades
  let response = await p2p.execute.add_asset({
    action: {
      to_trade:{
        trade_id: 1
      }
    },
    asset:{
        cw721_coin:{
          address:nft_contract.address,
          token_id: token_id_response.tokens[0]
        }
      }
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
