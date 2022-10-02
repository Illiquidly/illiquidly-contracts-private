import { Address } from '../terra_utils';
import { env } from '../env_helper';

function getContractLog(response: any) {
  console.log(response);
  return response.logs[0].eventsByType.from_contract;
}

async function main() {
  // Getting a handler for the current address
  let handler = new Address(env['mnemonics'][0]);


  let cw721_tokens = env['cw721'];
  let cw721_token_names = Object.keys(cw721_tokens);

  let p2p = handler.getContract(env.contracts.p2p)
  // We try to add trades
  let response = await p2p.execute.add_n_f_ts_wanted({
    trade_id: 2,
    //nfts_wanted: [cw721_tokens[cw721_token_names[0]]]
    nfts_wanted: ["terra14dcwvg4zplrc28g5q3802n2mmnp3fsp2yh7mn7gkxssnrjqp4ycq676kqf"]
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
