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
  let nft_contract = handler.getContract(cw721_tokens[cw721_token_names[0]]);

  let p2p = handler.getContract(env.contracts.p2p)

  // We try to remove the nft from the cont_ract

  let response = await p2p.execute.remove_assets({
    trade_id: 1,
    assets:[[1,{
        cw721_coin:{
          address:nft_contract.address,
          token_id: "mario_the_best153783208"
        }
      }]]
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
