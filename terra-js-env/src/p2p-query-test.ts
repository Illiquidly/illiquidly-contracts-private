import { Address } from './terra_utils';
import { env } from './env_helper';
import { MsgExecuteContract } from '@terra-money/terra.js';

function getContractLog(response: any) {
  return response.logs[0].eventsByType.from_contract;
}

async function main() {
  // Getting a handler for the current address
  let handler = new Address(env['mnemonics'][0]);
  let counter = new Address(env['mnemonics'][1]);
  console.log(counter.getAddress(), handler.getAddress());
  let cw20_tokens = env['cw20'];
  let cw20_token_names = Object.keys(cw20_tokens);
  let iliq_token_id = 0;

  let iliq = handler.getContract(cw20_tokens[cw20_token_names[0]]);
  let iliq_counter = counter.getContract(cw20_tokens[cw20_token_names[0]]);
  let p2p = handler.getContract(env.contracts.p2p);
  let p2p_counter = counter.getContract(env.contracts.p2p);
  let fee = handler.getContract(env.contracts.fee);
  let fee_counter = counter.getContract(env.contracts.fee);

  let gal = "terra1w876nfvxr9xz9my7x85jcwtxuqae4ptjpjwj7v";
  let punk = "terra1pk646xtdgwym74k46cajttdu6uvypa5jw5wa3j"
  let nft_contract = handler.getContract(punk);

  console.log(await fee.query.contract_info());
  
  let response: any;
  response = await p2p.query.contract_info();
  console.log(response);

  let c = true;
  let start_after: number | undefined = undefined;
  while (c) {
    //console.log(start_after)
    response = await p2p.query.get_all_trades({
      start_after: start_after,
      limit: 20,
      filters: {
        states: ['Countered']
      }
    });
    console.log(response);
    if (response && response.trades && response.trades.length > 0) {
      start_after = response.trades[response.trades.length - 1].trade_id;
    } else {
      c = false;
    }
  }

  console.log("All counters now : ");
  c = true;
  start_after = undefined;
  while (c) {
    let msg: any = {
      limit: 10,
      trade_id: 1001,
      filters: {
        states: ['Published']
      },
      start_after: start_after
    };
    response = await p2p.query.get_counter_trades(msg);
    console.log(response);
    if (
      response &&
      response.counter_trades &&
      response.counter_trades.length > 0
    ) {
      start_after =
        response.counter_trades[response.counter_trades.length - 1].counter_id;
    } else {
      c = false;
    }
  }

  console.log("All counterers, like all of them now : ");
  c = true;
  let start_after_any: any = undefined;
  while (c) {
    let msg: any = {
      limit: 19,
      start_after: start_after_any
    };
    response = await p2p.query.get_all_counter_trades(msg);
    console.log(response);
    if (
      response &&
      response.counter_trades &&
      response.counter_trades.length > 0
    ) {
      start_after_any = {
        trade_id:
          response.counter_trades[response.counter_trades.length - 1].trade_id,
        counter_id:
          response.counter_trades[response.counter_trades.length - 1].counter_id
      };
    } else {
      c = false;
    }
  }

  console.log("All trades, like all of them now : ");
  c = true;
  start_after = undefined;
   while (c) {
    //console.log(start_after)
    response = await p2p.query.get_all_trades({
      start_after: start_after,
      limit: 20,
      filters: {
      }
    });
    console.log(response);
    if (response && response.trades && response.trades.length > 0) {
      start_after = response.trades[response.trades.length - 1].trade_id;
    } else {
      c = false;
    }
  }
  
}

main()
  .then((resp) => {
    console.log(resp);
  })
  .catch((err) => {
    console.log(err);
  });
