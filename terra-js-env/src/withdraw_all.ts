import { Address } from './terra_utils';
import { env } from './env_helper';
import { MsgExecuteContract } from '@terra-money/terra.js';

const asyncFilter = async (arr: any[], predicate: any) => {
	const results = await Promise.all(arr.map(predicate));
	return arr.filter((_v, index) => results[index]);
}

async function main() {
  // Getting a handler for the current address
  let handler = new Address(env['mnemonics'][1]);

  let p2p = handler.getContract(env.contracts.p2p);
  let fee = handler.getContract(env.contracts.fee);

  let response: any;


  // We start by withdrawing trades (and cancelling them)
  let c = true;
  let start_after: number | undefined = undefined;
  let withdraw = [];
  let cancel_and_withdraw = [];
  let pay_fee_and_withdraw = [];


  while (c) {
    response = await p2p.query.get_all_trades({
      start_after: start_after,
      limit: 20,
      filters: {
        owner: handler.getAddress()
      }
    });
    withdraw.push(
      ...response.trades.filter((trade: any) => {
        return (trade.trade_info && 
          (trade.trade_info.state == 'created' ||
            trade.trade_info.state == 'cancelled') &&
          !trade.trade_info.assets_withdrawn
        );
      })
    );
    cancel_and_withdraw.push(
      ...response.trades.filter((trade: any) => {
        return trade.trade_info && (trade.trade_info.state == 'published' || trade.trade_info.state == "countered");
      })
    );

		pay_fee_and_withdraw.push(
	      ...await asyncFilter(response.trades, async (trade: any) => {
	  	if(trade.trade_info && trade.trade_info.state == 'accepted'){
	  		// We verify the funds are not already withdrawn
	  		let counter_id = trade.trade_info.accepted_info.counter_id;
	  		let response = await p2p.query.counter_trade_info({
	  			trade_id: trade.trade_id,
	  			counter_id: counter_id
	  		});
	  		return !response.assets_withdrawn;
	  	}
	  	return false;
	  }));

    if (response && response.trades && response.trades.length > 0) {
      start_after = response.trades[response.trades.length - 1].trade_id;
    } else {
      c = false;
    }
  }

  
  console.log("To simply withdraw", withdraw.length);
  let txArray = withdraw.map((trade) => {
    return new MsgExecuteContract(
      handler.getAddress(), // sender
      p2p.address, // contract account address
      {
        withdraw_all_from_trade: {
          trade_id: trade.trade_id
        }
      } // handle msg
    );
  });
  if (txArray.length > 0) {
    response = await p2p.execute.executeSome(txArray);
  }

	console.log("To cancel and withdraw", cancel_and_withdraw.length)
  txArray = cancel_and_withdraw.flatMap((trade) => 
    [new MsgExecuteContract(
      handler.getAddress(), // sender
      p2p.address, // contract account address
      {
        cancel_trade: {
          trade_id: trade.trade_id
        }
      } // handle msg
    ),
    new MsgExecuteContract(
      handler.getAddress(), // sender
      p2p.address, // contract account address
      {
        withdraw_all_from_trade: {
          trade_id: trade.trade_id
        }
      } // handle msg
    )
    ]
  );
  if (txArray.length > 0) {
    response = await p2p.execute.executeSome(txArray);
  }

	console.log("To withdraw accepted trades", pay_fee_and_withdraw.length)
  txArray = await Promise.all(pay_fee_and_withdraw.map(async (trade) => {

  	// First we verify the funds are not already withdrawn


		response = await fee.query.fee({
			trade_id: trade.trade_id
		})
		return new MsgExecuteContract(
      handler.getAddress(), // sender
      fee.address, // contract account address
      {
        pay_fee_and_withdraw: {
          trade_id: trade.trade_id
        }
      }, // handle msg
      {"uusd": response.fee}
    )
	}
	));

	if (txArray.length > 0) {
    response = await p2p.execute.executeSome(txArray);
  }


  // We do the same with counter trades

  c = true;
  let start_after_any: any = undefined;
  withdraw = [];
  cancel_and_withdraw = [];
  pay_fee_and_withdraw = [];

  while (c) {
  	let msg: any = {
      limit: 30,
      filters: {
        owner: handler.getAddress()
      },
      start_after: start_after_any
    };
    response = await p2p.query.get_all_counter_trades(msg);
    withdraw.push(
      ...response.counter_trades.filter((trade: any) => {
        return (trade.trade_info && 
          (trade.trade_info.state == 'created' ||
            trade.trade_info.state == 'cancelled') &&
          !trade.trade_info.assets_withdrawn
        );
      })
    );
    cancel_and_withdraw.push(
      ...response.counter_trades.filter((trade: any) => {
        return trade.trade_info && trade.trade_info.state == 'published';
      })
    );

		pay_fee_and_withdraw.push(
	      ...await asyncFilter(response.counter_trades, async (trade: any) => {
	  	if(trade.trade_info && trade.trade_info.state == 'accepted'){
	  		// We verify the funds are not already withdrawn
	  		let response = await p2p.query.trade_info({
	  			trade_id: trade.trade_id,
	  		});
	  		return !response.assets_withdrawn;
	  	}
	  	return false;
	  }));

    if (response && response.counter_trades && response.counter_trades.length > 0) {
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

  
  console.log("To simply withdraw", withdraw.length);
  txArray = withdraw.map((trade) => {
    return new MsgExecuteContract(
      handler.getAddress(), // sender
      p2p.address, // contract account address
      {
        withdraw_all_from_counter: {
          trade_id: trade.trade_id,
          counter_id: trade.counter_id
        }
      } // handle msg
    );
  });
  if (txArray.length > 0) {
    response = await p2p.execute.executeSome(txArray);
  }

	console.log("To cancel and withdraw", cancel_and_withdraw.length)
  txArray = cancel_and_withdraw.flatMap((trade) => 
    [new MsgExecuteContract(
      handler.getAddress(), // sender
      p2p.address, // contract account address
      {
        cancel_counter_trade: {
          trade_id: trade.trade_id,
          counter_id: trade.counter_id,
        }
      } // handle msg
    ),
    new MsgExecuteContract(
      handler.getAddress(), // sender
      p2p.address, // contract account address
      {
        withdraw_all_from_counter: {
          trade_id: trade.trade_id,
          counter_id: trade.counter_id,
        }
      } // handle msg
    )
    ]
  );
  if (txArray.length > 0) {
    response = await p2p.execute.executeSome(txArray);
  }

	console.log("To withdraw accepted trades", pay_fee_and_withdraw.length)
  txArray = await Promise.all(pay_fee_and_withdraw.map(async (trade) => {

  	// First we verify the funds are not already withdrawn
		response = await fee.query.fee({
			trade_id: trade.trade_id
		})
		return new MsgExecuteContract(
      handler.getAddress(), // sender
      fee.address, // contract account address
      {
        pay_fee_and_withdraw: {
          trade_id: trade.trade_id
        }
      }, // handle msg
      {"uusd": response.fee}
    )
	}
	));

	if (txArray.length > 0) {
    response = await p2p.execute.executeSome(txArray);
  }
}

main()
  .then((resp) => {
    console.log(resp);
  })
  .catch((err) => {
    console.log(err);
  });
