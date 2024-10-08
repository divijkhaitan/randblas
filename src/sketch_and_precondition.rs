use nalgebra::{DMatrix, DVector};
use crate::sketch::{sketching_operator, DistributionType};
use crate::cg;
// use crate::sketch_and_solve::sketched_least_squares_qr;
// use crate::solvers::{solve_diagonal_system, solve_upper_triangular_system};

fn compute_vector_norm(x:&DMatrix<f64>) -> f64{
    let mut norm = 0.0;
    if x.nrows() == 1{
        for i in 0..x.ncols(){
            norm+= (x[(i, 0)]).powf(2.0);
        }
    }
    else if x.ncols() == 1{
        for i in 0..x.nrows(){
            norm+= (x[(i, 0)]).powf(2.0);
        }
    }
    else{
        println!("Not a vector");
    }
    norm
}

fn compute_vector_norm_in_matrix(y:&DMatrix<f64>, a:&DMatrix<f64>) -> f64{
    let x = y.transpose()*a*y;
    let mut norm = 0.0;
    if x.nrows() == 1{
        for i in 0..x.ncols(){
            norm+= (x[(0, i)]).powf(2.0);
        }
    }
    else if x.ncols() == 1{
        for i in 0..x.nrows(){
            norm+= (x[(0, i)]).powf(2.0);
        }
    }
    else{
        println!("Not a vector");
    }
    norm
}

pub fn blendenpik_least_squares_overdetermined(a:&DMatrix<f64>, b:&DMatrix<f64>, epsilon:f64, l:usize, sampling_factor:f64) -> DMatrix<f64>
{
    let d = if sampling_factor*(a.ncols() as f64) > a.nrows() as f64 {a.nrows()} else {(sampling_factor*(a.ncols() as f64)).floor() as usize};
    let s = sketching_operator(DistributionType::Gaussian, d, a.nrows());
    let a_sk = &s*a;
    let b_sk = &s*b;
    let (q, r) = a_sk.qr().unpack();
    println!("b shape :({} {}), q shape: ({} {})",b_sk.nrows(), b_sk.ncols(), q.nrows(), q.ncols());
    let z_0 = q.transpose()*&b_sk;
    let rinv = r.try_inverse().unwrap();
    let a_preconditioned = a*&rinv;
    // TODO: Replace with Iterative solver
    let z = cg::cgls(&a_preconditioned, &b, epsilon, l, Some(z_0));
    rinv*z
}

fn lsrn(a: &DMatrix<f64>, b: &DMatrix<f64>, sampling_factor: f64, epsilon: f64, l: usize) -> DMatrix<f64> {
    let m = a.nrows();
    let n = a.ncols();
    
    let d = (sampling_factor * n as f64).ceil() as usize;
    let s = sketching_operator(DistributionType::Gaussian, d, m);
    let a_sk = &s * a;

    let svd_obj = a_sk.svd(false, true);
    let sigma = DMatrix::from_diagonal(&svd_obj.singular_values);
    let v = svd_obj.v_t.unwrap().transpose();
    
    let mut sigma_inv = DMatrix::zeros(n, n);
    for i in 0..n {
        if sigma[i] > 0.0 {
            sigma_inv[(i, i)] = 1.0 / sigma[i];
        }
    }
    let n = v*&sigma_inv;
    let a_precond = a*&n;
    let mut y_hat = DMatrix::zeros(sigma_inv.ncols(), 1);
    y_hat = cg::cgls(&a_precond, b, epsilon, l,  Some(y_hat), );
    n* y_hat
}


pub fn lsrn_least_squares_overdetermined(a:&DMatrix<f64>, b:&DMatrix<f64>, epsilon:f64, l:usize, sampling_factor:f64) -> DMatrix<f64>
{
    let d = if sampling_factor*(a.ncols() as f64) > a.nrows() as f64 {a.nrows()} else {(sampling_factor*(a.ncols() as f64)).floor() as usize};
    let s = sketching_operator(DistributionType::Gaussian, d, a.nrows());
    let a_sk = &s*a;
    let b_sk = &s*b;
    let (q, r) = a_sk.qr().unpack();
    println!("b shape :({} {}), q shape: ({} {})",b_sk.nrows(), b_sk.ncols(), q.nrows(), q.ncols());
    let z_0 = q.transpose()*&b_sk;
    let rinv = r.try_inverse().unwrap();
    let a_preconditioned = a*&rinv;
    let z = cg::cgls(&a_preconditioned, b, epsilon, l, Some(z_0));
    rinv*z
}

#[cfg(test)]
mod tests
{
    use rand::Rng;
    use rand_distr::{Uniform, Normal, Distribution};
    use nalgebra::DMatrix;
    use crate::{sketch::{sketching_operator, DistributionType}, sketch_and_precondition::{blendenpik_least_squares_overdetermined, lsrn_least_squares_overdetermined}, solvers::{solve_upper_triangular_system, solve_diagonal_system}};
    use std::time::Instant;
    #[test]
    fn test_sketch_and_precondition_overdetermined_least_squares()
    {
        // This code is to generate a random hypothesis, and add generate noisy data from that hypothesis
        let mut rng = rand::thread_rng();
        let n = rand::thread_rng().gen_range(100..150);
        let m = rand::thread_rng().gen_range(500..5000);
        let epsilon = 0.01;
        let normal = Normal::new(0.0, epsilon).unwrap();
        let uniform = Uniform::new(-100.0, 100.0);
        let hypothesis = DMatrix::from_fn(n, 1, |_i, _j| uniform.sample(&mut rng));
        let mut data = sketching_operator(DistributionType::Gaussian, m, n);
        let y = &data*&hypothesis;
        for i in 0..m {
            let noise_vector = DMatrix::from_fn(n, 1, |_, _| normal.sample(&mut rng));
            for j in 0..n {
                data[(i, j)] += noise_vector[(j, 0)];
            }
        }
        
        // Blendenpik Test
        let start1 = Instant::now();
        // compute using sketched qr
        let x = blendenpik_least_squares_overdetermined(&data, &y, 0.1, 10000, 1.5);
        let duration1 = start1.elapsed();
        // compute using plain qr
        let start2 = Instant::now();
        let (q, r) = data.clone().qr().unpack();
        let b_transformed = q.transpose()*&y;
        let actual_solution = solve_upper_triangular_system(&r, &b_transformed);
        let duration2 = start2.elapsed();
        let residual_minimum = &data*&hypothesis - &y;
        let residual_sketch = &data*x - &y;
        let residual_actual = &data*actual_solution - &y;
        println!("Hypothesis residual: {}, Sketched Residual: {}, Actual Residual: {}", (residual_minimum).norm(), (residual_sketch).norm(), (residual_actual).norm());
        println!("Time for sketched algorithm vs time for qr factorisation: {:.2?} {:.2?} ", duration1, duration2);

        // LSRN
        let start1 = Instant::now();
        // compute using lsrn
        let x = lsrn_least_squares_overdetermined(&data, &y, 0.1, 10000, 1.5);
        let duration1 = start1.elapsed();
        
        // compute using SVD
        let start2 = Instant::now();
        let svd_obj = data.clone().svd(true, true);
        let u = svd_obj.u.unwrap();
        let sigma = DMatrix::from_diagonal(&svd_obj.singular_values);
        let v = svd_obj.v_t.unwrap().transpose();
        let b_transformed = u.transpose()*&y;
        let actual_solution = v*solve_diagonal_system(&sigma, &b_transformed);
        let duration2 = start2.elapsed();
        let residual_minimum = &data*&hypothesis - &y;
        let residual_sketch = &data*x - &y;
        let residual_actual = &data*actual_solution - &y;
        println!("Hypothesis residual: {}, Sketched Residual: {}, Actual Residual: {}", (residual_minimum).norm(), (residual_sketch).norm(), (residual_actual).norm());
        println!("Time for sketched algorithm vs time for SVD: {:.2?} {:.2?} ", duration1, duration2);

    }
    
}