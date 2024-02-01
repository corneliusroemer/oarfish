use crate::util::oarfish_types::TranscriptInfo;
use crate::util::count_function::bin_transcript_normalize_counts;
use statrs::function::gamma::ln_gamma;
use rayon::prelude::*;


pub fn binomial_probability(interval_count: Vec<f32>, interval_length: Vec<f32>, distinct_rate: f64, tlen:usize) -> Vec<f64>{

    let interval_counts = interval_count;
    let interval_lengths = interval_length;

    if interval_counts.iter().sum::<f32>() == 0.0 {
        return vec![0.0; tlen + 1];
    }

    if distinct_rate == 0.0 {
        return vec![0.0; tlen + 1];
    }
    //eprintln!("counts: {:?}", interval_counts);
    //eprintln!("length: {:?}", interval_lengths);
    let mut probabilities: Vec<f64> = interval_counts
                                  .iter()
                                  .zip(interval_lengths.iter())
                                  .map(|(&count, &length)| {
                                  if count ==0.0 || length == 0.0 {
                                      0.0 
                                  } else {
                                      //eprintln!("count: {:?}, length: {:?}, rate:{:?}", count, length, distinct_rate);
                                      (count as f64) / (length as f64 * distinct_rate)
                                  }
                                  }).collect();
    //eprintln!("prob: {:?}", probabilities);
    let sum_vec = interval_counts.iter().sum::<f32>();
    let log_numerator1: f64 = ln_gamma(sum_vec as f64 + 1.0);
    let log_denominator: Vec<f64> =  interval_counts.iter().map(|&count| ln_gamma(count as f64 + 1.0) + ln_gamma((sum_vec - count) as f64 + 1.0)).collect();
    let log_numerator2: Vec<f64> = probabilities.iter().zip(interval_counts.iter()).map(|(&prob, &count)| {
        let num2 = if prob > 10e-8_f64 {prob.ln() * (count as f64)} else {(10e-8_f64).ln() * (count as f64)};
        if num2.is_nan() || num2.is_infinite() {
            eprintln!("num2 is: {:?}", num2);
            eprintln!("prob and count is: {:?}\t {:?}", prob, count);
            panic!("Incorrect result. multinomial_probability function provides nan or infinite values for log_numerator2");
        }
        num2
    }).collect();
    
    let log_numerator3: Vec<f64> = probabilities.iter().zip(interval_counts.iter()).map(|(&prob, &count)| {
        let num3 = if (1.0 - prob) > 10e-8_f64 {(1.0 - prob).ln() * (sum_vec - count) as f64} else {(10e-8_f64).ln() * (sum_vec - count) as f64};
        if num3.is_nan() || num3.is_infinite() {
            eprintln!("num3 is: {:?}", num3);
            eprintln!("prob and sum_vec and count is: {:?}\t {:?}\t {:?}", prob, sum_vec, count);
            panic!("Incorrect result. multinomial_probability function provides nan or infinite values for log_numerator3");
        }
        num3
    }).collect();


    let mut result: Vec<f64> = log_denominator.iter().zip(log_numerator2.iter().zip(log_numerator3.iter()))
                                                    .map(|(denom,(num2, num3))| {
                                                        let res = (log_numerator1 - denom + num2 +num3).exp();
                                                        if res.is_nan() || res.is_infinite() {
                                                            panic!("Incorrect result. multinomial_probability function provides nan or infinite values for result");
                                                        }
                                                        res
                                                    }).collect();
    //eprintln!("result: {:?}", result);
    //eprintln!("result is: {:?}", result);
    let bin_length = interval_lengths[0];
    let num_bins = interval_lengths.len() as u32;      
    // Compute the sum of probabilities
    let sum: f64 = result.iter().sum();
    // Normalize the probabilities by dividing each element by the sum
    let normalized_prob: Vec<f64> = result.iter().map(|&prob| prob / (bin_length as f64 * sum)).collect();
    //eprintln!("normalized_prob: {:?}", normalized_prob);

    let mut prob_vec = vec![0.0; tlen + 1];
    let mut bin_start = 0;
    for i in 0..num_bins {
        //let bin_start = i * interval_lengths[i as usize];
        let start_index = bin_start;
        let bin_end = (i + 1) as f32 * interval_lengths[i as usize];
    
        //let start_index = if i == 0 { bin_start } else { bin_start + 1 };
        let end_index = if i + 1 == num_bins { (tlen + 1) as u32 } else { bin_end.floor() as u32 };
    
        prob_vec[start_index as usize..end_index as usize]
            .iter_mut()
            .for_each(|v| *v = normalized_prob[i as usize]);

        bin_start = end_index;
    }

    let cdf: Vec<f64> = prob_vec.iter().scan(0.0, |acc, &prob| {
    *acc += prob;
    Some(*acc)
    }).collect();

    //eprintln!("cdf: {:?}", cdf);
    
    cdf    
    
}


pub fn binomial_continuous_prob(txps: &mut Vec<TranscriptInfo>, bins: &u32, threads: usize) {

    rayon::ThreadPoolBuilder::new()
    .num_threads(threads)
    .build()
    .unwrap();


    txps.par_iter_mut().enumerate().for_each(|(i, t)| {
        
        let mut temp_prob: Vec<f32>;

        if *bins != 0 {
            //eprintln!("in multinomial prob");
            let bin_counts: Vec<f32>;
            let bin_lengths: Vec<f32>;
            let mut num_discarded_read_temp: usize = 0;
            let bin_coverage: Vec<f64>;
            (bin_counts, bin_lengths, num_discarded_read_temp, bin_coverage) = bin_transcript_normalize_counts(t, bins); //binning the transcript length and obtain the counts and length vectors
            //==============================================================================================
            
            let tlen = t.len.get(); //transcript length
            let distinct_rate: f64 =  bin_counts.iter().zip(bin_lengths.iter()).map(|(&count, &length)| (count as f64)/ (length as f64)).sum();
            let prob_dr: Vec<f64> = binomial_probability(bin_counts, bin_lengths, distinct_rate, tlen);
            temp_prob = prob_dr.iter().map(|&x| x as f32).collect();

        } else {
            //not binning the transcript length
            let len = t.len.get() as u32; //transcript length

            //obtain the start and end of reads aligned to these transcripts
            let mut start_end_ranges: Vec<u32> = t.ranges.iter().map(|range| vec![range.start, range.end]).flatten().collect();
            start_end_ranges.push(0); // push the first position of the transcript
            start_end_ranges.push(len); // push the last position of the transcript
            start_end_ranges.sort(); // Sort the vector in ascending order
            start_end_ranges.dedup(); // Remove consecutive duplicates
            //convert the sorted vector of starts and ends into a vector of consecutive ranges
            let distinct_interval: Vec<std::ops::Range<u32>> = start_end_ranges.windows(2).map(|window| window[0]..window[1]).collect(); 
            let interval_length : Vec<u32> = start_end_ranges.windows(2).map(|window| window[1] - window[0]).collect(); 
            //obtain the number of reads aligned in each distinct intervals
            let mut interval_counts: Vec<f32> = Vec::new();
            for interval in distinct_interval
            {
                interval_counts.push(t.ranges.iter().filter(|range| range.start <= interval.start && range.end >= interval.end).count() as f32);
            }


            let tlen = t.len.get(); //transcript length
            let distinct_rate: f64 =  interval_counts.iter().zip(interval_length.iter()).map(|(&count, &length)| (count as f64)/ (length as f64)).sum();
            let prob_dr: Vec<f64> = binomial_probability(interval_counts, interval_length.iter().map(|&len| len as f32).collect(), distinct_rate, tlen);
            temp_prob = prob_dr.iter().map(|&x| x as f32).collect();
        }

        t.coverage_prob = temp_prob;
    });


}